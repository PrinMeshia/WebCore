//! Site-root output renderers: SEO files (`robots.txt`, `sitemap.xml`,
//! `feed.xml`) and PWA assets (`manifest.webmanifest`, `sw.js`).
//!
//! Pure string renderers, unit-tested here; `cli/build.rs` orchestrates when
//! each file is written.

use super::config::{Feed, Pwa};

// ── PWA assets ───────────────────────────────────────────────────────────────

/// A minimal offline-capable service worker: network-first for same-origin GETs,
/// caching each success and falling back to the cache (then `/`) when offline.
pub(super) const SERVICE_WORKER_JS: &str = "const C='webcore-pwa-v1';\
self.addEventListener('install',function(e){self.skipWaiting();});\
self.addEventListener('activate',function(e){e.waitUntil(caches.keys().then(function(ks){return Promise.all(ks.filter(function(k){return k!==C;}).map(function(k){return caches.delete(k);}));}).then(function(){return self.clients.claim();}));});\
self.addEventListener('fetch',function(e){var r=e.request;if(r.method!=='GET'||new URL(r.url).origin!==location.origin)return;\
e.respondWith(fetch(r).then(function(res){var cp=res.clone();caches.open(C).then(function(c){c.put(r,cp);});return res;}).catch(function(){return caches.match(r).then(function(m){return m||caches.match('/');});}));});\n";

/// JSON-escape a manifest string value (quotes and backslashes).
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Render `manifest.webmanifest` from the resolved `[pwa]` config + icon paths.
pub(super) fn render_manifest(
    pwa: &Pwa,
    icon_192: &str,
    icon_512: &str,
    icon_maskable: &str,
) -> String {
    format!(
        "{{\n  \"name\": \"{name}\",\n  \"short_name\": \"{short}\",\n  \
         \"start_url\": \"/\",\n  \"scope\": \"/\",\n  \"display\": \"{display}\",\n  \
         \"background_color\": \"{bg}\",\n  \"theme_color\": \"{theme}\",\n  \"icons\": [\n    \
         {{ \"src\": \"{i192}\", \"sizes\": \"192x192\", \"type\": \"image/png\" }},\n    \
         {{ \"src\": \"{i512}\", \"sizes\": \"512x512\", \"type\": \"image/png\" }},\n    \
         {{ \"src\": \"{imask}\", \"sizes\": \"512x512\", \"type\": \"image/png\", \"purpose\": \"maskable\" }}\n  ]\n}}\n",
        name = json_escape(&pwa.name),
        short = json_escape(&pwa.short_name),
        display = json_escape(&pwa.display),
        bg = json_escape(&pwa.background_color),
        theme = json_escape(&pwa.theme_color),
        i192 = json_escape(icon_192),
        i512 = json_escape(icon_512),
        imask = json_escape(icon_maskable),
    )
}

// ── SEO root files ───────────────────────────────────────────────────────────

/// Render `robots.txt`: allow-all, plus a `Sitemap:` line when a site URL is set.
pub(super) fn render_robots(url: Option<&str>) -> String {
    let mut s = String::from("User-agent: *\nAllow: /\n");
    if let Some(base) = url {
        s.push_str(&format!("\nSitemap: {base}/sitemap.xml\n"));
    }
    s
}

/// Render `sitemap.xml` from an absolute base URL and clean-URL routes
/// (`"/"`, `"/skills/"`, …). Routes under `/404` are excluded.
pub(super) fn render_sitemap(base: &str, routes: &[String]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for route in routes.iter().filter(|r| !r.starts_with("/404")) {
        s.push_str(&format!("  <url><loc>{base}{route}</loc></url>\n"));
    }
    s.push_str("</urlset>\n");
    s
}

/// Minimal XML entity escaping for feed text nodes.
pub(super) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Render an RSS 2.0 feed from a data collection. Items are sorted by the date
/// field (descending — string compare works for ISO-8601 dates) and truncated
/// to `feed.limit`. Item links are `base + link_prefix + <link_field value>`.
pub(super) fn render_feed(feed: &Feed, base: &str, items: &[serde_json::Value]) -> String {
    let field = |item: &serde_json::Value, name: &str| -> String {
        match item.get(name) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            _ => String::new(),
        }
    };
    let mut rows: Vec<&serde_json::Value> = items.iter().collect();
    rows.sort_by_key(|item| std::cmp::Reverse(field(item, &feed.date_field)));
    if let Some(n) = feed.limit {
        rows.truncate(n);
    }

    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\">\n  <channel>\n",
    );
    s.push_str(&format!("    <title>{}</title>\n", xml_escape(&feed.title)));
    s.push_str(&format!("    <link>{}</link>\n", xml_escape(base)));
    s.push_str(&format!(
        "    <description>{}</description>\n",
        xml_escape(&feed.description)
    ));
    for item in rows {
        let link = format!(
            "{base}{}{}",
            feed.link_prefix,
            field(item, &feed.link_field)
        );
        let date = field(item, &feed.date_field);
        let summary = field(item, &feed.summary_field);
        s.push_str("    <item>\n");
        s.push_str(&format!(
            "      <title>{}</title>\n",
            xml_escape(&field(item, &feed.title_field))
        ));
        s.push_str(&format!("      <link>{}</link>\n", xml_escape(&link)));
        s.push_str(&format!("      <guid>{}</guid>\n", xml_escape(&link)));
        if !date.is_empty() {
            s.push_str(&format!("      <pubDate>{}</pubDate>\n", xml_escape(&date)));
        }
        if !summary.is_empty() {
            s.push_str(&format!(
                "      <description>{}</description>\n",
                xml_escape(&summary)
            ));
        }
        s.push_str("    </item>\n");
    }
    s.push_str("  </channel>\n</rss>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_has_required_fields_and_icons() {
        let pwa = Pwa {
            name: "My \"App\"".to_string(),
            short_name: "App".to_string(),
            theme_color: "#7C3AED".to_string(),
            background_color: "#05030F".to_string(),
            display: "standalone".to_string(),
        };
        let m = render_manifest(
            &pwa,
            "/assets/icon-192.png",
            "/assets/icon-512.png",
            "/assets/icon-maskable.png",
        );
        assert!(m.contains(r#""short_name": "App""#));
        assert!(m.contains(r#""start_url": "/""#));
        assert!(m.contains(r#""display": "standalone""#));
        assert!(m.contains(r##""theme_color": "#7C3AED""##));
        assert!(m.contains(r#""sizes": "192x192""#));
        assert!(m.contains(r#""purpose": "maskable""#));
        // Quotes in the name are JSON-escaped.
        assert!(
            m.contains(r#""name": "My \"App\"""#),
            "name not escaped:\n{m}"
        );
    }

    #[test]
    fn robots_without_url_has_no_sitemap_line() {
        let out = render_robots(None);
        assert!(out.contains("User-agent: *"));
        assert!(out.contains("Allow: /"));
        assert!(!out.contains("Sitemap:"));
    }

    #[test]
    fn robots_with_url_links_sitemap() {
        let out = render_robots(Some("https://example.com"));
        assert!(out.contains("Sitemap: https://example.com/sitemap.xml"));
    }

    #[test]
    fn sitemap_lists_routes_and_skips_404() {
        let routes = vec!["/".to_string(), "/skills/".to_string(), "/404/".to_string()];
        let out = render_sitemap("https://example.com", &routes);
        assert!(out.contains("<loc>https://example.com/</loc>"));
        assert!(out.contains("<loc>https://example.com/skills/</loc>"));
        assert!(!out.contains("/404"), "404 must not be advertised:\n{out}");
        assert!(out.trim_start().starts_with("<?xml"));
        assert!(out.contains("</urlset>"));
    }
}
