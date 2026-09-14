//! Dev server: HTTP file serving with WebSocket HMR (hot-module reload).

use super::build::build_project;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, TcpListener, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Request, Response, Server};
use tungstenite::{accept, Message};

pub(crate) type WsClients = Arc<Mutex<Vec<tungstenite::WebSocket<std::net::TcpStream>>>>;

/// WebSocket HMR client script injected into HTML pages in dev mode
fn get_ws_hmr_script(ws_port: u16) -> String {
    format!(
        r"<script>
(function(){{
  var wsPort={ws_port};
  function connect(){{
    var ws=new WebSocket('ws://'+location.hostname+':'+wsPort);
    ws.onmessage=function(e){{if(e.data==='reload'){{location.reload();}}}};
    ws.onclose=function(){{setTimeout(connect,1000);}};
    ws.onerror=function(){{ws.close();}};
  }}
  connect();
}})();
</script>"
    )
}

/// Resolve a URL path to a file inside `dist/`, rejecting path traversal attempts.
/// Returns `None` if the resolved path escapes the dist root.
pub(crate) fn resolve_safe_path(url: &str) -> Option<PathBuf> {
    let dist_root = fs::canonicalize("dist").ok()?;

    let candidate = if url == "/" {
        PathBuf::from("dist/index.html")
    } else if url.contains('.') {
        // Strip leading slash to avoid treating it as absolute
        PathBuf::from("dist").join(url.trim_start_matches('/'))
    } else {
        let html_path = PathBuf::from("dist")
            .join(url.trim_start_matches('/'))
            .join("index.html");
        if html_path.exists() {
            html_path
        } else {
            PathBuf::from("dist/index.html")
        }
    };

    // Canonicalize resolves `..` and symlinks; verify the result stays inside dist/
    let canonical = fs::canonicalize(&candidate).ok()?;
    if canonical.starts_with(&dist_root) {
        Some(canonical)
    } else {
        None // path traversal attempt
    }
}

pub(crate) fn handle_request(request: Request, ws_port: u16) -> Result<(), String> {
    let url = request.url();
    // Strip query string (?v=... cache-busting, etc.) before resolving to a file path
    let url = url.split('?').next().unwrap_or(url);

    let file_path = match resolve_safe_path(url) {
        Some(p) => p,
        None => {
            let response = tiny_http::Response::from_string("403 Forbidden")
                .with_status_code(tiny_http::StatusCode(403));
            let _ = request.respond(response);
            return Ok(());
        }
    };

    if let Ok(mut bytes) = fs::read(&file_path) {
        let content_type = match file_path.extension().and_then(|e| e.to_str()).unwrap_or("") {
            "html" => {
                // Inject WebSocket HMR script
                if let Ok(html) = String::from_utf8(bytes.clone()) {
                    let hmr_script = get_ws_hmr_script(ws_port);
                    let injected_html = html.replace("</body>", &format!("{hmr_script}</body>"));
                    bytes = injected_html.into_bytes();
                }
                "text/html; charset=utf-8"
            }
            "css" => "text/css; charset=utf-8",
            "js" => "application/javascript; charset=utf-8",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "woff" | "woff2" => "font/woff2",
            "json" => "application/json",
            _ => "application/octet-stream",
        };
        let response = Response::from_data(bytes)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                    .expect("Content-Type header name and value are always valid ASCII"),
            )
            .with_header(
                tiny_http::Header::from_bytes(&b"Cache-Control"[..], b"no-cache")
                    .expect("Cache-Control header name and value are always valid ASCII"),
            );
        request
            .respond(response)
            .map_err(|e| format!("respond error: {e}"))
    } else {
        let response = Response::from_string("Not Found").with_status_code(404);
        request
            .respond(response)
            .map_err(|e| format!("respond error: {e}"))
    }
}

/// Resolve the address the dev server binds to, from the optional `--host` value.
///
/// `--host` used to be display-only: the listeners were hard-coded to
/// `0.0.0.0`, so `webc dev --host 127.0.0.1` printed a reassuring loopback URL
/// while still accepting requests from every machine on the network. The flag
/// now decides where we actually listen, which means an unparseable value has
/// to be an error — silently falling back to the wildcard is exactly the
/// mismatch this is fixing.
pub(crate) fn resolve_bind_ip(host: Option<&str>) -> Result<IpAddr, String> {
    match host {
        None => Ok(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
        Some(h) => h.parse::<IpAddr>().map_err(|_| {
            format!("--host attend une adresse IP (ex: 0.0.0.0, 127.0.0.1, ::1) — reçu '{h}'")
        }),
    }
}

fn bind_server_with_fallback(
    ip: IpAddr,
    start_port: u16,
    max_tries: u16,
) -> Result<(Server, u16), String> {
    let mut port = start_port;
    for _ in 0..max_tries {
        match Server::http((ip, port)) {
            Ok(server) => return Ok((server, port)),
            Err(e) => {
                let kind = e
                    .as_ref()
                    .downcast_ref::<std::io::Error>()
                    .map(std::io::Error::kind);
                match kind {
                    Some(std::io::ErrorKind::AddrInUse) => {
                        port = port.saturating_add(1);
                        continue;
                    }
                    // The requested address belongs to no interface on this
                    // machine — a bad `--host`, not a busy port. Retrying on
                    // the next port would loop 50 times over the same failure.
                    Some(std::io::ErrorKind::AddrNotAvailable) => {
                        return Err(format!(
                            "impossible d'écouter sur {ip} : cette adresse n'est portée par aucune interface de cette machine"
                        ))
                    }
                    _ => return Err(format!("server error: {e}")),
                }
            }
        }
    }
    Err(format!(
        "no free port in range {}..{}",
        start_port,
        start_port.saturating_add(max_tries)
    ))
}

/// Render an IP as it must appear in a URL — IPv6 literals need brackets,
/// otherwise the `:` of the address collides with the port separator.
pub(crate) fn format_host(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => format!("[{v6}]"),
    }
}

fn get_primary_ipv4() -> Option<IpAddr> {
    if let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) {
        if socket.connect(("8.8.8.8", 80)).is_ok() {
            if let Ok(addr) = socket.local_addr() {
                if let IpAddr::V4(ipv4) = addr.ip() {
                    if !ipv4.is_loopback() {
                        return Some(IpAddr::V4(ipv4));
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn serve_project(
    port: u16,
    host: Option<String>,
    auto_open: bool,
) -> Result<(), String> {
    // Resolve `--host` before building: a typo in the flag should fail in a
    // second, not after a full compilation.
    let bind_ip = resolve_bind_ip(host.as_deref())?;

    // initial build
    build_project(None).map_err(|e| e.to_string())?;

    // Shared list of connected WebSocket clients
    let ws_clients: WsClients = Arc::new(Mutex::new(Vec::new()));

    // start file watcher
    let rebuild_flag = Arc::new(Mutex::new(false));
    let flag_clone = rebuild_flag.clone();

    let mut watcher: RecommendedWatcher = notify::recommended_watcher(
        move |res: std::result::Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        if let Ok(mut f) = flag_clone.lock() {
                            *f = true;
                        }
                    }
                    _ => {}
                }
            }
        },
    )
    .map_err(|e| format!("watcher error: {e}"))?;

    watcher
        .watch(Path::new("src"), RecursiveMode::Recursive)
        .map_err(|e| format!("watch error: {e}"))?;
    if Path::new("theme.toml").exists() {
        watcher
            .watch(Path::new("theme.toml"), RecursiveMode::NonRecursive)
            .map_err(|e| format!("watch error: {e}"))?;
    }
    watcher
        .watch(Path::new("webc.toml"), RecursiveMode::NonRecursive)
        .map_err(|e| format!("watch error: {e}"))?;

    // start HTTP server with port auto-increment if in use
    let (server, bound_port) = bind_server_with_fallback(bind_ip, port, 50)?;
    let ws_port = bound_port + 1;

    // The HMR socket follows the HTTP server: restricting one while leaving
    // the other on the wildcard would keep the process reachable anyway.
    let ws_listener =
        TcpListener::bind((bind_ip, ws_port)).map_err(|e| format!("WS bind error: {e}"))?;

    // A wildcard bind has no address of its own to show, so the local URL is
    // spelled `localhost`; any other bind is displayed as-is.
    let local_host = if bind_ip.is_unspecified() {
        "localhost".to_string()
    } else {
        format_host(bind_ip)
    };
    println!("🚀 Dev server running at:");
    println!("  Local:   http://{local_host}:{bound_port}");
    println!("  HMR:     WebSocket on ws://{local_host}:{ws_port}");

    // Only advertise a Network URL when the server really answers on it: the
    // wildcard reaches the LAN address, a loopback bind reaches nothing else.
    let network_ip = if bind_ip.is_unspecified() {
        get_primary_ipv4()
    } else if bind_ip.is_loopback() {
        None
    } else {
        Some(bind_ip)
    };
    let mut qr_url: Option<String> = None;
    if let Some(ip) = network_ip {
        let url = format!("http://{}:{bound_port}", format_host(ip));
        println!("  Network: {url}");
        qr_url = Some(url);
    }

    // auto-open browser
    if auto_open {
        let open_url = format!("http://{local_host}:{bound_port}");
        let _ = open::that_detached(open_url);
    }

    // print QR code for network URL if available
    if let Some(url) = qr_url {
        if let Ok(code) = qrcode::QrCode::new(url.as_bytes()) {
            println!("\n  Scan QR (Network):");
            let qr = code
                .render::<qrcode::render::unicode::Dense1x2>()
                .quiet_zone(true)
                .build();
            println!("{qr}");
        }
    }

    // spawn WebSocket accept loop — adds each new client to the shared list
    let ws_clients_for_accept = ws_clients.clone();
    thread::spawn(move || {
        for stream in ws_listener.incoming() {
            match stream {
                Ok(tcp) => {
                    let _ = tcp.set_nonblocking(false);
                    match accept(tcp) {
                        Ok(ws) => {
                            if let Ok(mut list) = ws_clients_for_accept.lock() {
                                list.push(ws);
                            }
                        }
                        Err(e) => eprintln!("WS handshake error: {e}"),
                    }
                }
                Err(e) => eprintln!("WS accept error: {e}"),
            }
        }
    });

    // spawn rebuild loop — after each successful rebuild, broadcast "reload"
    let ws_clients_for_rebuild = ws_clients.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(200));
        let mut do_rebuild = false;
        if let Ok(mut f) = rebuild_flag.lock() {
            if *f {
                do_rebuild = true;
                *f = false;
            }
        }
        if do_rebuild {
            println!("♻️  Rebuilding...");
            match build_project(None) {
                Ok(()) => {
                    println!("🔄 HMR: broadcasting reload to connected clients");
                    if let Ok(mut clients) = ws_clients_for_rebuild.lock() {
                        clients.retain_mut(|ws| ws.send(Message::Text("reload".into())).is_ok());
                    }
                }
                Err(e) => eprintln!("Rebuild failed: {e}"),
            }
        }
    });

    // serve loop
    for request in server.incoming_requests() {
        if let Err(e) = handle_request(request, ws_port) {
            eprintln!("request error: {e}");
        }
    }

    Ok(())
}
