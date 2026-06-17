//! Full-build integration tests.
//!
//! Each test copies a real example project from `../examples/` into a fresh
//! temporary directory, runs the compiled `webc` binary (`build`) there, and
//! validates the emitted `dist/`:
//!   - the build exits successfully,
//!   - the expected entry files exist,
//!   - the emitted JavaScript is syntactically valid (checked with `node --check`
//!     when Node.js is available),
//!   - building twice produces byte-identical output (determinism).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static TEMP_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Path to the compiled `webc` binary under test.
fn webc_bin() -> &'static str {
    env!("CARGO_BIN_EXE_webcore-compiler")
}

/// Repository-level `examples/` directory.
fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples")
}

/// Create a unique scratch directory for one test run.
fn scratch_dir(label: &str) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("webcore-it-{}-{}-{}", std::process::id(), n, label));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean stale scratch dir");
    }
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Recursively copy a project, skipping build artifacts.
fn copy_project(src: &Path, dst: &Path) {
    for entry in fs::read_dir(src).expect("read example dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        if name == "dist" || name == "target" || name == "node_modules" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if from.is_dir() {
            fs::create_dir_all(&to).expect("create subdir");
            copy_project(&from, &to);
        } else {
            fs::copy(&from, &to).expect("copy file");
        }
    }
}

/// Run `webc build` in `project_dir`, asserting success.
fn run_build(project_dir: &Path) {
    let output = Command::new(webc_bin())
        .arg("build")
        .current_dir(project_dir)
        .output()
        .expect("spawn webc build");
    assert!(
        output.status.success(),
        "`webc build` failed in {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        project_dir.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Collect every file under `root` as path → contents (sorted, for comparison).
fn snapshot_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files);
    files
}

fn collect_files(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(dir).expect("read dist dir") {
        let entry = entry.expect("dist entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else {
            let rel = path
                .strip_prefix(root)
                .expect("strip dist root")
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, fs::read(&path).expect("read dist file"));
        }
    }
}

/// Validate JS syntax with `node --check` when Node.js is available.
fn check_js_syntax(js_path: &Path) {
    let node = Command::new("node").arg("--version").output();
    if node.is_err() {
        eprintln!("note: node not found, skipping JS syntax check");
        return;
    }
    let output = Command::new("node")
        .arg("--check")
        .arg(js_path)
        .output()
        .expect("spawn node --check");
    assert!(
        output.status.success(),
        "emitted JS is not syntactically valid: {}\n{}",
        js_path.display(),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Build one example end-to-end and validate the emitted dist/.
fn build_example(example: &str) {
    let src = examples_dir().join(example);
    assert!(src.is_dir(), "missing example project: {}", src.display());

    let work = scratch_dir(example);
    copy_project(&src, &work);
    run_build(&work);

    let dist = work.join("dist");
    assert!(
        dist.join("index.html").is_file(),
        "{example}: dist/index.html missing"
    );
    assert!(
        dist.join("assets/theme.css").is_file(),
        "{example}: dist/assets/theme.css missing"
    );

    // Validate every emitted JS file.
    let first = snapshot_tree(&dist);
    assert!(!first.is_empty(), "{example}: dist/ is empty");
    for rel in first.keys() {
        if rel.ends_with(".js") {
            check_js_syntax(&dist.join(rel));
        }
    }

    // Determinism: a second build from the same sources must be byte-identical.
    run_build(&work);
    let second = snapshot_tree(&dist);
    assert_eq!(
        first.keys().collect::<Vec<_>>(),
        second.keys().collect::<Vec<_>>(),
        "{example}: file set changed between two identical builds"
    );
    for (rel, bytes) in &first {
        assert_eq!(
            bytes, &second[rel],
            "{example}: dist/{rel} differs between two identical builds"
        );
    }

    fs::remove_dir_all(&work).ok();
}

/// Generate a synthetic project on disk: `components` components and `pages`
/// pages, each page using several components, with state, events, @if and @for.
fn write_synthetic_project(root: &Path, components: usize, pages: usize) {
    fs::create_dir_all(root.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(root.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(root.join("src/components")).expect("mkdir components");

    fs::write(
        root.join("webc.toml"),
        "[app]\ntitle = \"Synthetic\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");

    fs::write(
        root.join("src/layouts/MainLayout.webc"),
        "layout MainLayout {\n    header { h1 \"Synthetic\" }\n    main { slot content }\n    footer { p \"footer\" }\n}\n",
    )
    .expect("write layout");

    for i in 0..components {
        let src = format!(
            r#"component Comp{i} {{
    state {{
        count{i}: Number = {i}
    }}
    view {{
        div class="comp-{i}" {{
            p "Composant {i} : {{count{i}}}"
            button on:click={{count{i} += 1}} {{ "+" }}
            @if count{i} > 10 {{
                span "beaucoup"
            }} @else {{
                span "peu"
            }}
        }}
    }}
    style {{
        .comp-{i} {{ padding: {i}px; }}
    }}
}}
"#
        );
        fs::write(root.join(format!("src/components/Comp{i}.webc")), src).expect("write component");
    }

    for p in 0..pages {
        let name = if p == 0 {
            "home".to_string()
        } else {
            format!("doc{p}")
        };
        let mut src = format!("page \"{name}\" {{\n    h2 \"Page {p}\"\n");
        // Each page instantiates 5 components, spread across the set.
        for k in 0..5 {
            let c = (p * 5 + k) % components;
            src.push_str(&format!("    Comp{c} {{}}\n"));
        }
        src.push_str("}\n");
        fs::write(root.join(format!("src/pages/{name}.webc")), src).expect("write page");
    }
}

/// Performance guard: a 50-component / 20-page project must build well under
/// a generous ceiling. Catches pathological complexity regressions (e.g.
/// accidental O(n²) passes), not micro-variations. Prints the measured time.
#[test]
fn perf_synthetic_50_components_20_pages() {
    let work = scratch_dir("synthetic");
    write_synthetic_project(&work, 50, 20);

    let started = std::time::Instant::now();
    run_build(&work);
    let elapsed = started.elapsed();
    eprintln!("synthetic build (50 components, 20 pages): {elapsed:?}");

    let dist = work.join("dist");
    assert!(dist.join("index.html").is_file(), "dist/index.html missing");
    assert!(
        dist.join("doc19/index.html").is_file(),
        "dist/doc19/index.html missing"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(60),
        "synthetic build took {elapsed:?} — pathological slowdown (limit: 60s)"
    );

    fs::remove_dir_all(&work).ok();
}

/// Regression: `webc:img` must inject width/height read from the real image
/// in `public/` during an actual `webc build` (the generation used to run
/// with `project_root = None`, silently skipping dimension injection).
#[test]
fn webc_img_injects_dimensions_from_public() {
    // Minimal valid 1x1 RGBA PNG.
    const PNG_1X1: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 252, 207, 192, 80,
        15, 0, 4, 133, 1, 128, 132, 169, 140, 33, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    let work = scratch_dir("webcimg");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("public")).expect("mkdir public");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n    img webc:img=true src=\"/pixel.png\" alt=\"px\"\n}\n",
    )
    .expect("write page");
    fs::write(work.join("public/pixel.png"), PNG_1X1).expect("write png");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("read index.html");
    assert!(
        html.contains("width=\"1\"") && html.contains("height=\"1\""),
        "webc:img did not inject dimensions:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// Production-mode build: the prod pipeline (HTML/CSS/JS minification, SRI,
/// inlined critical CSS, deferred stylesheet, CSP meta) was previously only
/// covered by partial golden tests, never end-to-end.
#[test]
fn full_build_prod_mode_counter() {
    let src = examples_dir().join("counter");
    let work = scratch_dir("counter-prod");
    copy_project(&src, &work);
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"Compteur WebCore\"\nlang = \"fr\"\nmode = \"prod\"\ncsp = true\n",
    )
    .expect("write prod webc.toml");

    run_build(&work);
    let dist = work.join("dist");
    let html = fs::read_to_string(dist.join("index.html")).expect("read index.html");

    // SRI on the runtime script and stylesheet.
    assert!(
        html.contains("integrity=\"sha256-") && html.contains("crossorigin=\"anonymous\""),
        "prod build is missing SRI attributes:\n{html}"
    );
    // Critical CSS inlined in <head>, full stylesheet deferred.
    assert!(
        html.contains("<style>") && html.contains("data-webcore-defer"),
        "prod build is missing inlined critical CSS / deferred stylesheet:\n{html}"
    );
    // Strict CSP meta (csp = true).
    assert!(
        html.contains("http-equiv=\"Content-Security-Policy\""),
        "prod build with csp=true is missing the CSP meta tag:\n{html}"
    );
    // Minified HTML: comments stripped.
    assert!(
        !html.contains("<!--"),
        "prod HTML still contains comments:\n{html}"
    );
    // No inline event handlers (CSP-safe delegation only).
    assert!(
        !html.contains("onclick=\"") && !html.contains("onsubmit=\""),
        "prod HTML contains inline event handlers:\n{html}"
    );

    // Minified JS must still be syntactically valid.
    // The filename is content-hashed (e.g. webcore.abc12345.js), so locate it dynamically.
    let webcore_js_path = fs::read_dir(dist.join("assets"))
        .expect("read assets dir")
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("webcore.") && n.ends_with(".js"))
                .unwrap_or(false)
        })
        .expect("find hashed webcore.*.js in dist/assets");
    let js = fs::read_to_string(&webcore_js_path).expect("read webcore.js");
    assert!(
        !js.lines().any(|l| l.trim_start().starts_with("//")),
        "prod JS still contains line comments"
    );
    check_js_syntax(&webcore_js_path);

    // Prod builds must be deterministic too (hashes depend only on content).
    let first = snapshot_tree(&dist);
    run_build(&work);
    let second = snapshot_tree(&dist);
    assert_eq!(
        first, second,
        "prod dist/ differs between two identical builds"
    );

    fs::remove_dir_all(&work).ok();
}

#[test]
fn full_build_counter() {
    build_example("counter");
}

#[test]
fn full_build_todo() {
    build_example("todo");
}

#[test]
fn full_build_blog() {
    build_example("blog");
}

#[test]
fn full_build_forms() {
    build_example("forms");
}

#[test]
fn full_build_i18n() {
    build_example("i18n");
}

#[test]
fn full_build_docs() {
    build_example("docs");
}

/// `webc check --json` must emit one machine-readable JSON line on stdout:
/// parse errors carry file/line/col, reference issues a stable code, and a
/// healthy project reports ok:true with exit code 0.
#[test]
fn check_json_structured_diagnostics() {
    let work = scratch_dir("checkjson");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");

    let run_check = |dir: &Path| -> (bool, serde_json::Value) {
        let out = Command::new(webc_bin())
            .args(["check", "--json"])
            .current_dir(dir)
            .output()
            .expect("spawn webc check --json");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("stdout is not valid JSON ({e}):\n{stdout}"));
        (out.status.success(), parsed)
    };

    // 1. Parse error → positioned diagnostic, non-zero exit.
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n    div {\n        p \"oops\n}\n",
    )
    .expect("write broken page");
    let (ok, report) = run_check(&work);
    assert!(!ok, "broken project must exit non-zero");
    assert_eq!(report["ok"], false);
    let d = &report["diagnostics"][0];
    assert_eq!(d["severity"], "error");
    assert_eq!(d["code"], "parse");
    assert!(
        d["file"].as_str().unwrap_or("").ends_with("home.webc"),
        "parse diagnostic should point at home.webc: {report}"
    );
    assert!(
        d["line"].as_u64().unwrap_or(0) > 0,
        "line missing: {report}"
    );
    assert!(d["col"].as_u64().unwrap_or(0) > 0, "col missing: {report}");

    // 2. Unknown component → stable code, no position required.
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n    Missing {}\n}\n",
    )
    .expect("write page with unknown component");
    let (ok, report) = run_check(&work);
    assert!(!ok);
    assert_eq!(report["diagnostics"][0]["code"], "unknown-component");

    // 3. Healthy project → ok:true, empty diagnostics, exit 0.
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n    p \"ok\"\n}\n",
    )
    .expect("write valid page");
    let (ok, report) = run_check(&work);
    assert!(ok, "valid project must exit 0: {report}");
    assert_eq!(report["ok"], true);
    assert_eq!(report["diagnostics"].as_array().map(Vec::len), Some(0));

    fs::remove_dir_all(&work).ok();
}
