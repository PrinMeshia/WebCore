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
        "page \"home\" {\n    img webc:img=true src=\"/assets/pixel.png\" alt=\"px\"\n}\n",
    )
    .expect("write page");
    fs::write(work.join("public/pixel.png"), PNG_1X1).expect("write png");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("read index.html");
    assert!(
        html.contains("width=\"1\"") && html.contains("height=\"1\""),
        "webc:img did not inject dimensions:\n{html}"
    );
    // Dimensions and a URL that resolves are not a choice: the same `src` must
    // get both. It used to get one or the other depending on which convention
    // the author had guessed.
    for url in srcset_and_src_urls(&html) {
        let rel = url.trim_start_matches("/assets/");
        assert!(
            work.join("dist/assets").join(rel).is_file(),
            "{url} is referenced but missing from dist/assets:\n{html}"
        );
    }

    fs::remove_dir_all(&work).ok();
}

/// Regression: rewriting fingerprinted references must touch references, not
/// text. A page documenting the asset pipeline had the `/assets/…` of its own
/// code sample silently rewritten to the hashed name by a whole-file
/// `String::replace`.
#[test]
fn asset_rewrite_leaves_page_text_alone() {
    const PNG_1X1: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 252, 207, 192, 80,
        15, 0, 4, 133, 1, 128, 132, 169, 140, 33, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    let work = scratch_dir("asset-text");
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
    fs::write(work.join("public/pixel.png"), PNG_1X1).expect("write png");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n    img src=\"/assets/pixel.png\" alt=\"px\"\n    p \"/assets/pixel.png\"\n}\n",
    )
    .expect("write page");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("read index.html");

    // The attribute is rewritten…
    assert!(
        html.contains("<img src=\"/assets/pixel.")
            && !html.contains("<img src=\"/assets/pixel.png\""),
        "the src attribute should carry the fingerprinted name:\n{html}"
    );
    // …and the prose naming the same path is left exactly as written.
    assert!(
        html.contains("<p>/assets/pixel.png</p>"),
        "page text must not be rewritten:\n{html}"
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
    // #48 — parse diagnostics carry a stable `WCxxxx` code.
    assert!(
        d["code"].as_str().unwrap_or("").starts_with("WC"),
        "parse diagnostic should carry a WC code: {report}"
    );
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

// ── #44: pipeline non-regression across every example, in dev AND prod ───────
//
// Builds each example project in both modes and asserts invariants that guard
// the bug classes found while dogfooding:
//   - compiled `()=>…` closures must never leak into prerendered HTML (an
//     interpolation rendering its closure source instead of the value);
//   - the runtime JS must never contain a double-wrapped `()=>()=>` closure (#52);
//   - the emitted JS must be syntactically valid (node --check);
//   - each mode must be deterministic (byte-identical rebuild).

/// Build one example project in the given mode (`"dev"` / `"prod"`).
fn run_build_mode(project_dir: &Path, mode: &str) {
    let flag = format!("--{mode}");
    let output = Command::new(webc_bin())
        .args(["build", &flag])
        .current_dir(project_dir)
        .output()
        .expect("spawn webc build");
    assert!(
        output.status.success(),
        "`webc build {flag}` failed in {}\n--- stderr ---\n{}",
        project_dir.display(),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Locate the shared runtime JS asset (`webcore*.js`) in a dist/ tree, if any.
fn find_runtime_js(dist: &Path) -> Option<PathBuf> {
    fs::read_dir(dist.join("assets"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("webcore") && n.ends_with(".js"))
                .unwrap_or(false)
        })
}

/// Invariants every example must satisfy in every build mode.
fn assert_pipeline_invariants(dist: &Path, mode: &str, example: &str) {
    // Prerendered HTML must never carry a raw compiled closure — those live only
    // in the runtime JS. A leak means an interpolation rendered its closure
    // source as text instead of the evaluated value.
    for (rel, bytes) in snapshot_tree(dist) {
        if rel.ends_with(".html") {
            let html = String::from_utf8_lossy(&bytes);
            assert!(
                !html.contains("()=>S.get") && !html.contains("()=>STORE.get"),
                "{example} [{mode}]: compiled closure leaked into HTML {rel}"
            );
        }
    }
    // The runtime JS must be valid and free of double-wrapped closures (#52).
    if let Some(js_path) = find_runtime_js(dist) {
        let js = fs::read_to_string(&js_path).expect("read runtime js");
        assert!(
            !js.contains("()=>()=>"),
            "{example} [{mode}]: double-wrapped _e closure in {}",
            js_path.display()
        );
        check_js_syntax(&js_path);
    }
}

/// Build an example in both modes, checking invariants + determinism.
fn check_example_pipeline(example: &str) {
    let src = examples_dir().join(example);
    assert!(src.is_dir(), "missing example project: {}", src.display());
    for mode in ["dev", "prod"] {
        let work = scratch_dir(&format!("{example}-{mode}"));
        copy_project(&src, &work);
        run_build_mode(&work, mode);
        let dist = work.join("dist");
        assert!(
            dist.join("index.html").is_file(),
            "{example} [{mode}]: dist/index.html missing"
        );
        assert_pipeline_invariants(&dist, mode, example);
        let first = snapshot_tree(&dist);
        run_build_mode(&work, mode);
        let second = snapshot_tree(&dist);
        assert_eq!(
            first, second,
            "{example} [{mode}]: dist/ differs between two identical builds"
        );
        fs::remove_dir_all(&work).ok();
    }
}

#[test]
fn pipeline_invariants_all_examples_dev_and_prod() {
    let mut examples: Vec<String> = fs::read_dir(examples_dir())
        .expect("read examples/ dir")
        .flatten()
        .filter(|e| e.path().join("webc.toml").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    examples.sort();
    assert!(!examples.is_empty(), "no example projects found");
    for ex in &examples {
        check_example_pipeline(ex);
    }
}

#[test]
fn check_a11y_reports_and_respects_strict() {
    let work = scratch_dir("a11y-check");
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
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n    img src=\"/a.svg\"\n}\n",
    )
    .expect("write page with img-no-alt");

    let run = |args: &[&str]| -> (bool, serde_json::Value) {
        let out = Command::new(webc_bin())
            .args(args)
            .current_dir(&work)
            .output()
            .expect("spawn webc check");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("stdout is not valid JSON ({e}):\n{stdout}"));
        (out.status.success(), parsed)
    };

    // Plain check: a missing alt is not a hard error → passes.
    let (ok, report) = run(&["check", "--json"]);
    assert!(ok, "plain check should pass:\n{report}");

    // --a11y: the img-alt warning is reported but does NOT fail (no --strict).
    let (ok, report) = run(&["check", "--a11y", "--json"]);
    assert!(
        ok,
        "--a11y warnings must not fail without --strict:\n{report}"
    );
    assert_eq!(report["diagnostics"][0]["code"], "a11y-img-alt");
    assert_eq!(report["diagnostics"][0]["severity"], "warning");

    // --a11y --strict: the warning now fails the check.
    let (ok, _report) = run(&["check", "--a11y", "--strict", "--json"]);
    assert!(!ok, "--strict must fail on a11y findings");

    fs::remove_dir_all(&work).ok();
}

/// #70 — `webc check` verifies locale parity (a key present in one locale but
/// missing from another) and that every `t("key")` resolves; both are warnings
/// that only fail under `--strict`.
#[test]
fn check_i18n_parity_and_missing_keys() {
    let work = scratch_dir("i18n-check");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("locales")).expect("mkdir locales");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nlocale = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    // fr declares both keys; en is missing `tagline` → parity warning.
    fs::write(
        work.join("locales/fr.toml"),
        "welcome = \"Bienvenue\"\ntagline = \"Salut\"\n",
    )
    .expect("write fr");
    fs::write(work.join("locales/en.toml"), "welcome = \"Welcome\"\n").expect("write en");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    // References a resolvable key and an undefined one → missing-key warning.
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  h1 \"{t(\"welcome\")}\"\n  p \"{t(\"nope\")}\"\n}\n",
    )
    .expect("write page");

    let run = |args: &[&str]| -> (bool, serde_json::Value) {
        let out = Command::new(webc_bin())
            .args(args)
            .current_dir(&work)
            .output()
            .expect("spawn webc check");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("stdout is not valid JSON ({e}):\n{stdout}"));
        (out.status.success(), parsed)
    };

    // Plain check: parity + missing-key are warnings → does not fail.
    let (ok, report) = run(&["check", "--json"]);
    assert!(
        ok,
        "i18n warnings must not fail without --strict:\n{report}"
    );
    let codes: Vec<String> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        codes.iter().any(|c| c == "i18n-parity"),
        "expected i18n-parity warning, got: {codes:?}"
    );
    assert!(
        codes.iter().any(|c| c == "i18n-missing-key"),
        "expected i18n-missing-key warning, got: {codes:?}"
    );

    // --strict: the warnings now fail the check.
    let (ok, _report) = run(&["check", "--strict", "--json"]);
    assert!(!ok, "--strict must fail on i18n findings");

    fs::remove_dir_all(&work).ok();
}

/// #71 — `webc check` flags `public/` files referenced nowhere in the sources
/// (`orphan-asset`), ignoring internal `.md` docs, as a warning.
#[test]
fn check_orphan_public_assets() {
    let work = scratch_dir("orphan-check");
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
        "page \"home\" {\n  img src=\"/assets/used.svg\" alt=\"x\"\n}\n",
    )
    .expect("write page");
    fs::write(work.join("public/used.svg"), "<svg/>\n").expect("write used");
    fs::write(work.join("public/orphan.svg"), "<svg/>\n").expect("write orphan");
    fs::write(work.join("public/README.md"), "# internal\n").expect("write readme");

    let out = Command::new(webc_bin())
        .args(["check", "--json"])
        .current_dir(&work)
        .output()
        .expect("spawn webc check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let report: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON on stdout");

    // Passes (warning only), and flags exactly the orphan.
    assert!(
        out.status.success(),
        "orphan warning must not fail plain check"
    );
    let orphans: Vec<String> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "orphan-asset")
        .map(|d| d["message"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        orphans.iter().any(|m| m.contains("orphan.svg")),
        "orphan.svg should be flagged, got: {orphans:?}"
    );
    assert!(
        !orphans.iter().any(|m| m.contains("used.svg")),
        "used.svg must not be flagged: {orphans:?}"
    );
    assert!(
        !orphans.iter().any(|m| m.contains("README.md")),
        "internal .md must not be flagged: {orphans:?}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #46 — `[i18n] static` generates one page per locale (default at root, others
/// under `/{locale}/`) with correct `lang`, translated content, and hreflang
/// alternates; the sitemap lists every localized URL.
#[test]
fn i18n_static_generates_localized_pages() {
    let work = scratch_dir("i18n-static");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("locales")).expect("mkdir locales");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nlocale = \"fr\"\nmode = \"prod\"\nurl = \"https://example.com\"\n\n[i18n]\nstatic = true\n",
    )
    .expect("write webc.toml");
    fs::write(work.join("locales/fr.toml"), "welcome = \"Bienvenue\"\n").expect("write fr");
    fs::write(work.join("locales/en.toml"), "welcome = \"Welcome\"\n").expect("write en");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { h1 \"{t(\"welcome\")}\" }\n",
    )
    .expect("write home");

    run_build(&work);
    let dist = work.join("dist");

    let fr = fs::read_to_string(dist.join("index.html")).expect("fr index");
    let en = fs::read_to_string(dist.join("en/index.html")).expect("en index");

    assert!(fr.contains("<html lang=\"fr\">"), "fr lang wrong:\n{fr}");
    assert!(fr.contains("Bienvenue"), "fr content missing:\n{fr}");
    assert!(en.contains("<html lang=\"en\">"), "en lang wrong:\n{en}");
    assert!(en.contains("Welcome"), "en content missing:\n{en}");

    // Both pages advertise every locale + x-default.
    for html in [&fr, &en] {
        assert!(
            html.contains(r#"<link rel="alternate" hreflang="en" href="https://example.com/en/">"#),
            "en alternate missing:\n{html}"
        );
        assert!(
            html.contains(
                r#"<link rel="alternate" hreflang="x-default" href="https://example.com/">"#
            ),
            "x-default alternate missing:\n{html}"
        );
    }
    // Self-referencing canonicals.
    assert!(en.contains(r#"<link rel="canonical" href="https://example.com/en/">"#));

    // Sitemap lists the localized URL.
    let sitemap = fs::read_to_string(dist.join("sitemap.xml")).expect("sitemap");
    assert!(
        sitemap.contains("<loc>https://example.com/en/</loc>"),
        "sitemap missing localized URL:\n{sitemap}"
    );

    // The shared runtime initializes LOCALE from the pre-rendered <html lang>.
    let js = find_runtime_js(&dist).expect("runtime js");
    let js_src = fs::read_to_string(&js).expect("read runtime js");
    assert!(
        js_src.contains("document.documentElement.lang"),
        "runtime does not read html lang for LOCALE init"
    );

    fs::remove_dir_all(&work).ok();
}

/// #66 — `{t()}` interpolation in the `head {}` block: `<title>` and `<meta>`
/// values are resolved per-locale at build time, so each static locale page
/// gets indexable, localized metadata (not the default-locale strings).
#[test]
fn head_interpolation_localizes_title_and_meta() {
    let work = scratch_dir("head-i18n");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("locales")).expect("mkdir locales");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nlocale = \"fr\"\nmode = \"prod\"\nurl = \"https://example.com\"\n\n[i18n]\nstatic = true\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("locales/fr.toml"),
        "page_title = \"Accueil\"\npage_desc = \"Bienvenue sur mon site\"\n",
    )
    .expect("write fr");
    fs::write(
        work.join("locales/en.toml"),
        "page_title = \"Home\"\npage_desc = \"Welcome to my site\"\n",
    )
    .expect("write en");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  head {\n    title \"{t(\"page_title\")}\"\n    meta description = \"{t(\"page_desc\")}\"\n  }\n  h1 \"hi\"\n}\n",
    )
    .expect("write home");

    run_build(&work);
    let dist = work.join("dist");
    let fr = fs::read_to_string(dist.join("index.html")).expect("fr index");
    let en = fs::read_to_string(dist.join("en/index.html")).expect("en index");

    // Localized <title> per page.
    assert!(
        fr.contains("<title>Accueil</title>"),
        "fr title wrong:\n{fr}"
    );
    assert!(en.contains("<title>Home</title>"), "en title wrong:\n{en}");
    // Localized meta description per page.
    assert!(
        fr.contains(r#"<meta name="description" content="Bienvenue sur mon site">"#),
        "fr meta wrong:\n{fr}"
    );
    assert!(
        en.contains(r#"<meta name="description" content="Welcome to my site">"#),
        "en meta wrong:\n{en}"
    );
    // No unresolved interpolation leaks into the HTML.
    assert!(!fr.contains("t(\"page_title\")"), "fr leaked expr:\n{fr}");
    assert!(!en.contains("{t("), "en leaked brace:\n{en}");

    fs::remove_dir_all(&work).ok();
}

/// #67 — generic `link` items in `head {}` are emitted verbatim as `<link>`
/// tags (preload, rel=me, RSS alternate, …), attribute order preserved.
#[test]
fn head_generic_link_tags() {
    let work = scratch_dir("head-link");
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
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  head {\n    link rel=\"preload\" href=\"/f.woff2\" as=\"font\"\n    link rel=\"alternate\" type=\"application/rss+xml\" href=\"/feed.xml\"\n    link rel=\"me\" href=\"https://example.social/@me\"\n  }\n  h1 \"hi\"\n}\n",
    )
    .expect("write home");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");
    assert!(
        html.contains(r#"<link rel="preload" href="/f.woff2" as="font">"#),
        "preload link missing:\n{html}"
    );
    assert!(
        html.contains(r#"<link rel="alternate" type="application/rss+xml" href="/feed.xml">"#),
        "rss alternate link missing:\n{html}"
    );
    assert!(
        html.contains(r#"<link rel="me" href="https://example.social/@me">"#),
        "rel=me link missing:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #68 — declarative `jsonld {}` is serialised into a
/// `<script type="application/ld+json">` in the static HTML (crawler-visible,
/// no runtime JS), key order preserved and values JSON-escaped.
#[test]
fn head_jsonld_structured_data() {
    let work = scratch_dir("head-jsonld");
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
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  head {\n    jsonld {\n      \"@context\": \"https://schema.org\"\n      \"@type\": \"Person\"\n      name: \"Ada \\\"Lovelace\\\"\"\n      jobTitle: \"Engineer\"\n    }\n  }\n  h1 \"hi\"\n}\n",
    )
    .expect("write home");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");
    // Key order preserved; the embedded quote in the name is JSON-escaped.
    assert!(
        html.contains(
            r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"Person","name":"Ada \"Lovelace\"","jobTitle":"Engineer"}</script>"#
        ),
        "json-ld script missing or malformed:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #50 — `client="visible"` on a component instance wraps it in a static island
/// marker and emits valid deferred-hydration runtime (IntersectionObserver +
/// requestIdleCallback scheduler), with the component's on:mount deferred.
#[test]
fn islands_defer_hydration_end_to_end() {
    let work = scratch_dir("islands");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("src/components")).expect("mkdir components");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"prod\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/components/Counter.webc"),
        "component Counter {\n    state { count: Number = 0 }\n    on:mount { window.__m = 1 }\n    view { div { p \"C:{count}\" button on:click={count += 1} { \"+\" } } }\n}\n",
    )
    .expect("write component");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n    h1 \"Static\"\n    Counter client=\"visible\" {}\n}\n",
    )
    .expect("write home");

    run_build(&work);
    let dist = work.join("dist");

    // Island wrapper is present and the content is statically pre-rendered.
    let html = fs::read_to_string(dist.join("index.html")).expect("index");
    assert!(
        html.contains(r#"data-webcore-island="visible""#)
            && html.contains(r#"data-webcore-island-comp="Counter""#),
        "island marker missing:\n{html}"
    );
    assert!(
        html.contains("C:"),
        "island content not pre-rendered:\n{html}"
    );

    // Runtime carries the scheduler and is syntactically valid.
    let js = find_runtime_js(&dist).expect("runtime js");
    let js_src = fs::read_to_string(&js).expect("read runtime js");
    assert!(
        js_src.contains("IntersectionObserver") && js_src.contains("requestIdleCallback"),
        "island scheduler missing from runtime"
    );
    assert!(
        js_src.contains("data-webcore-ready"),
        "island hydration marker missing"
    );
    check_js_syntax(&js);

    fs::remove_dir_all(&work).ok();
}

/// #72 — `@for item in <data_import>` over a build-time data collection is
/// expanded into static markup at build time: fields become plain text /
/// static attributes, and no runtime `<template>` loop is emitted.
#[test]
fn data_collection_for_is_prerendered_static() {
    let work = scratch_dir("data-collection");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("data")).expect("mkdir data");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("data/projects.json"),
        "[{\"title\":\"Alpha\",\"url\":\"/p/alpha\"},{\"title\":\"Beta\",\"url\":\"/p/beta\"}]\n",
    )
    .expect("write data");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "import projects from \"data/projects.json\"\n\npage \"home\" {\n  @for project in projects {\n    div class=\"card\" {\n      h3 \"{project.title}\"\n      a href={project.url} { \"Voir\" }\n    }\n  }\n}\n",
    )
    .expect("write page");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");

    // Both items pre-rendered as static text + static hrefs.
    assert!(
        html.contains("<h3>Alpha</h3>"),
        "Alpha title missing:\n{html}"
    );
    assert!(
        html.contains("<h3>Beta</h3>"),
        "Beta title missing:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #69 — `[feed]` generates dist/feed.xml (RSS 2.0) from a data collection,
/// items sorted newest-first with absolute links, plus an auto-discovery
/// <link rel="alternate"> injected into every page head.
#[test]
fn feed_rss_generation_from_collection() {
    let work = scratch_dir("feed");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("data")).expect("mkdir data");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"Blog\"\nlang = \"fr\"\nmode = \"dev\"\nurl = \"https://example.com\"\n\n[feed]\ncollection = \"posts\"\ndescription = \"My posts\"\nlink_prefix = \"/post/\"\nlink_field = \"slug\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("data/posts.json"),
        "[{\"title\":\"Older\",\"slug\":\"older\",\"date\":\"2024-01-01\",\"summary\":\"a\"},{\"title\":\"Newer\",\"slug\":\"newer\",\"date\":\"2024-06-01\",\"summary\":\"b\"}]\n",
    )
    .expect("write data");
    fs::write(
        work.join("src/pages/home.webc"),
        "import posts from \"data/posts.json\"\n\npage \"home\" { h1 \"Blog\" }\n",
    )
    .expect("write page");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");

    run_build(&work);
    let dist = work.join("dist");
    let feed = fs::read_to_string(dist.join("feed.xml")).expect("feed.xml missing");

    assert!(
        html.contains("href=\"/p/alpha\""),
        "alpha href missing:\n{html}"
    );
    assert!(
        html.contains("href=\"/p/beta\""),
        "beta href missing:\n{html}"
    );
    // Two cards, fully static — no runtime loop template, no interpolation spans.
    assert_eq!(
        html.matches("class=\"card\"").count(),
        2,
        "expected 2 cards:\n{html}"
    );
    assert!(
        !html.contains("data-webcore-for"),
        "data collection must not emit a runtime for-loop template:\n{html}"
    );
    assert!(
        !html.contains("data-webcore-interpolation"),
        "collection fields must be fully static (no interpolation spans):\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #69 — `[feed]` generates dist/feed.xml (RSS 2.0) from a data collection,
/// items sorted newest-first with absolute links, plus an auto-discovery
/// <link rel="alternate"> injected into every page head.
#[test]
fn feed_rss_generation_from_collection() {
    let work = scratch_dir("feed");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("data")).expect("mkdir data");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"Blog\"\nlang = \"fr\"\nmode = \"dev\"\nurl = \"https://example.com\"\n\n[feed]\ncollection = \"posts\"\ndescription = \"My posts\"\nlink_prefix = \"/post/\"\nlink_field = \"slug\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("data/posts.json"),
        "[{\"title\":\"Older\",\"slug\":\"older\",\"date\":\"2024-01-01\",\"summary\":\"a\"},{\"title\":\"Newer\",\"slug\":\"newer\",\"date\":\"2024-06-01\",\"summary\":\"b\"}]\n",
    )
    .expect("write data");
    fs::write(
        work.join("src/pages/home.webc"),
        "import posts from \"data/posts.json\"\n\npage \"home\" { h1 \"Blog\" }\n",
    )
    .expect("write page");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");

    run_build(&work);
    let dist = work.join("dist");
    let feed = fs::read_to_string(dist.join("feed.xml")).expect("feed.xml missing");

    assert!(
        feed.contains("<rss version=\"2.0\">"),
        "not RSS 2.0:\n{feed}"
    );
    assert!(
        feed.contains("<link>https://example.com/post/newer</link>"),
        "absolute item link missing:\n{feed}"
    );
    // Newest first: "Newer" (2024-06) must appear before "Older" (2024-01).
    let pos_new = feed.find("Newer").expect("Newer missing");
    let pos_old = feed.find("Older").expect("Older missing");
    assert!(pos_new < pos_old, "items not sorted newest-first:\n{feed}");

    // Auto-discovery link injected into the page head.
    let html = fs::read_to_string(dist.join("index.html")).expect("index");
    assert!(
        html.contains(
            r#"<link rel="alternate" type="application/rss+xml" title="Blog" href="/feed.xml">"#
        ),
        "feed discovery link not injected:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #73 — a `markdown "file.md"` element renders the Markdown file to HTML at
/// build time and inlines it as static content (front-matter stripped).
#[test]
fn markdown_element_renders_at_build_time() {
    let work = scratch_dir("markdown");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("content")).expect("mkdir content");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("content/post.md"),
        "+++\ntitle = \"Hidden\"\n+++\n\n# Hello\n\nSome **bold** text and a [link](/x).\n",
    )
    .expect("write md");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  article { markdown \"content/post.md\" }\n}\n",
    )
    .expect("write page");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");

    assert!(
        html.contains("<h1>Hello</h1>"),
        "md heading missing:\n{html}"
    );
    assert!(
        html.contains("<strong>bold</strong>"),
        "md bold missing:\n{html}"
    );
    assert!(
        html.contains("<a href=\"/x\">link</a>"),
        "md link missing:\n{html}"
    );
    // Front-matter must not leak into the page.
    assert!(
        !html.contains("title = \"Hidden\""),
        "front-matter leaked:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// Every `/assets/…` URL carried by the `src` and `srcset` of the page's images.
fn srcset_and_src_urls(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for attr in ["src=\"", "srcset=\""] {
        let mut rest = html;
        while let Some(i) = rest.find(attr) {
            rest = &rest[i + attr.len()..];
            let Some(end) = rest.find('"') else { break };
            for entry in rest[..end].split(',') {
                if let Some(u) = entry.split_whitespace().next() {
                    if u.starts_with("/assets/") {
                        urls.push(u.to_string());
                    }
                }
            }
            rest = &rest[end..];
        }
    }
    urls
}

/// #74 — with `[images] widths`, `webc build` generates resized width variants
/// for raster `webc:img` sources and emits a `srcset` + `sizes`; the internal
/// marker attribute is removed.
#[test]
fn responsive_images_generate_variants_and_srcset() {
    let work = scratch_dir("responsive-img");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("public")).expect("mkdir public");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n\n[images]\nwidths = [40, 80]\nsizes = \"100vw\"\n",
    )
    .expect("write webc.toml");
    // A 100x50 source PNG → 40w and 80w variants (both < 100).
    let img = image::RgbImage::from_pixel(100, 50, image::Rgb([10, 20, 30]));
    img.save(work.join("public/hero.png")).expect("write png");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  img webc:img=true src=\"/assets/hero.png\" alt=\"Hero\"\n}\n",
    )
    .expect("write page");

    run_build(&work);
    let dist = work.join("dist");

    // Variant files generated next to the original.
    assert!(
        dist.join("assets/hero-40w.png").is_file(),
        "40w variant missing"
    );
    assert!(
        dist.join("assets/hero-80w.png").is_file(),
        "80w variant missing"
    );

    let html = fs::read_to_string(dist.join("index.html")).expect("index");
    assert!(
        html.contains("/assets/hero-40w.png 40w") && html.contains("/assets/hero-80w.png 80w"),
        "srcset variants missing:\n{html}"
    );
    // Every URL of the tag must name a file that exists. This assertion is the
    // one that was missing: the srcset used to carry `/assets/hero.png`, which
    // reads fine as a string and is a 404 on disk — the original is only ever
    // emitted under its fingerprinted name.
    for url in srcset_and_src_urls(&html) {
        let rel = url.trim_start_matches("/assets/");
        assert!(
            dist.join("assets").join(rel).is_file(),
            "{url} is referenced but no such file exists in dist/assets:\n{html}"
        );
    }
    assert!(
        !html.contains("/assets/hero.png"),
        "the un-fingerprinted original must not be referenced:\n{html}"
    );
    assert!(html.contains("sizes=\"100vw\""), "sizes missing:\n{html}");
    // The internal marker must be gone.
    assert!(
        !html.contains("data-webcore-img"),
        "responsive marker leaked into output:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #64 + #65 — the public/ copy skips internal `.md` docs, and images are only
/// emitted once (content-hashed), never also as their unhashed original.
#[test]
fn public_copy_skips_docs_and_dedupes_images() {
    let work = scratch_dir("publicfilter");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("public/logos")).expect("mkdir public/logos");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"prod\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    // Reference the image so it is fingerprinted and its ref rewritten.
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n    img src=\"/assets/logo.svg\" alt=\"logo\"\n}\n",
    )
    .expect("write page");
    fs::write(
        work.join("public/logo.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"></svg>\n",
    )
    .expect("write svg");
    // Internal docs that must NOT be deployed.
    fs::write(work.join("public/README.md"), "# internal\n").expect("write README");
    fs::write(work.join("public/logos/README.md"), "# logos\n").expect("write nested README");

    run_build(&work);
    let assets = work.join("dist/assets");

    // #64 — internal docs are not deployed.
    assert!(
        !assets.join("README.md").exists(),
        "public/README.md must not be deployed"
    );
    assert!(
        !assets.join("logos/README.md").exists(),
        "nested public README must not be deployed"
    );

    // #65 — the image exists once, content-hashed, and the unhashed original is
    // gone (no duplication).
    assert!(
        !assets.join("logo.svg").exists(),
        "unhashed image must be deduped away in favour of the fingerprinted copy"
    );
    let hashed: Vec<_> = fs::read_dir(&assets)
        .expect("read assets")
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("logo.") && n.ends_with(".svg"))
        .collect();
    assert_eq!(
        hashed.len(),
        1,
        "expected exactly one fingerprinted logo, found: {hashed:?}"
    );

    // The HTML reference points at the fingerprinted name.
    let html = fs::read_to_string(work.join("dist/index.html")).expect("read index.html");
    assert!(
        html.contains(&format!("/assets/{}", hashed[0])),
        "HTML should reference the fingerprinted image:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #57 — built-in functions: `math.*` calls compile to tree-shaken runtime
/// helpers, static calls fold at build time (SSG), and the runtime stays valid.
#[test]
fn builtins_math_tree_shake_and_ssg_fold() {
    let work = scratch_dir("builtins");
    fs::create_dir_all(work.join("src/layouts")).unwrap();
    fs::create_dir_all(work.join("src/pages")).unwrap();
    fs::create_dir_all(work.join("src/components")).unwrap();
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .unwrap();
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .unwrap();
    // Uses only math.round → math.clamp must be tree-shaken away.
    fs::write(
        work.join("src/components/Calc.webc"),
        "component Calc {\n    state { n: Number = 7 }\n    view { div {\n        p \"s:{math.round(2.4)}\"\n        p \"live:{math.round(n)}\"\n    } }\n}\n",
    )
    .unwrap();
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { Calc {} }\n",
    )
    .unwrap();

    run_build(&work);
    let dist = work.join("dist");

    // SSG folded the static call into the HTML.
    let html = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(
        html.contains(">2<"),
        "math.round(2.4) should fold to 2 at build time:\n{html}"
    );

    let js = find_runtime_js(&dist).expect("runtime js");
    let src = fs::read_to_string(&js).unwrap();
    // Used helper is emitted; the live call references it.
    assert!(
        src.contains("const _bmround="),
        "used builtin helper missing"
    );
    assert!(
        src.contains("_bmround(S.get('Calc__n'))"),
        "live builtin call missing"
    );
    // Unused helper is tree-shaken.
    assert!(
        !src.contains("_bmclamp"),
        "unused builtin must be tree-shaken"
    );
    check_js_syntax(&js);

    fs::remove_dir_all(&work).ok();
}

#[test]
fn builtins_str_tree_shake_and_ssg_fold() {
    let work = scratch_dir("builtins_str");
    fs::create_dir_all(work.join("src/layouts")).unwrap();
    fs::create_dir_all(work.join("src/pages")).unwrap();
    fs::create_dir_all(work.join("src/components")).unwrap();
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .unwrap();
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .unwrap();
    // Uses only str.slugify → str.upper must be tree-shaken away.
    fs::write(
        work.join("src/components/Slug.webc"),
        "component Slug {\n    state { title: String = \"Hello World\" }\n    view { div {\n        p \"s:{str.slugify(\"Hello, World!\")}\"\n        p \"live:{str.slugify(title)}\"\n    } }\n}\n",
    )
    .unwrap();
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { Slug {} }\n",
    )
    .unwrap();

    run_build(&work);
    let dist = work.join("dist");

    // SSG folded the static call into the HTML.
    let html = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(
        html.contains(">hello-world<"),
        "str.slugify(\"Hello, World!\") should fold to hello-world at build time:\n{html}"
    );

    let js = find_runtime_js(&dist).expect("runtime js");
    let src = fs::read_to_string(&js).unwrap();
    // Used helper is emitted; the live call references it.
    assert!(
        src.contains("const _bsslug="),
        "used builtin helper missing"
    );
    assert!(
        src.contains("_bsslug(S.get('Slug__title'))"),
        "live builtin call missing"
    );
    // Unused helper is tree-shaken.
    assert!(
        !src.contains("_bsupper"),
        "unused builtin must be tree-shaken"
    );
    check_js_syntax(&js);

    fs::remove_dir_all(&work).ok();
}

#[test]
fn builtins_fmt_runtime_only_no_i18n() {
    let work = scratch_dir("builtins_fmt");
    fs::create_dir_all(work.join("src/layouts")).unwrap();
    fs::create_dir_all(work.join("src/pages")).unwrap();
    fs::create_dir_all(work.join("src/components")).unwrap();
    // No [i18n] — fmt.* must still emit valid JS (LOCALE may be undefined).
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .unwrap();
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .unwrap();
    // Uses only fmt.number → fmt.currency must be tree-shaken away.
    fs::write(
        work.join("src/components/Price.webc"),
        "component Price {\n    state { qty: Number = 1234 }\n    view { div {\n        p \"n:{fmt.number(qty)}\"\n    } }\n}\n",
    )
    .unwrap();
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { Price {} }\n",
    )
    .unwrap();

    run_build(&work);
    let dist = work.join("dist");

    let js = find_runtime_js(&dist).expect("runtime js");
    let src = fs::read_to_string(&js).unwrap();
    // fmt.* is locale-dependent → runtime-only (never folded into HTML).
    assert!(
        src.contains("const _bfnum="),
        "used fmt builtin helper missing"
    );
    assert!(
        src.contains("_bfnum(S.get('Price__qty'))"),
        "live fmt builtin call missing"
    );
    // Guards against undefined LOCALE when i18n is not configured.
    assert!(
        src.contains("typeof LOCALE!=='undefined'"),
        "fmt helper must guard undefined LOCALE"
    );
    // Unused fmt helper is tree-shaken.
    assert!(
        !src.contains("_bfcur"),
        "unused fmt builtin must be tree-shaken"
    );
    check_js_syntax(&js);

    fs::remove_dir_all(&work).ok();
}

#[test]
fn builtins_arr_runtime_only_and_tree_shake() {
    let work = scratch_dir("builtins_arr");
    fs::create_dir_all(work.join("src/layouts")).unwrap();
    fs::create_dir_all(work.join("src/pages")).unwrap();
    fs::create_dir_all(work.join("src/components")).unwrap();
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .unwrap();
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .unwrap();
    // Uses only arr.sum → arr.unique must be tree-shaken away.
    fs::write(
        work.join("src/components/Bag.webc"),
        "component Bag {\n    state { items: Array = null }\n    view { div {\n        p \"total:{arr.sum(items)}\"\n    } }\n}\n",
    )
    .unwrap();
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { Bag {} }\n",
    )
    .unwrap();

    run_build(&work);
    let dist = work.join("dist");

    let js = find_runtime_js(&dist).expect("runtime js");
    let src = fs::read_to_string(&js).unwrap();
    assert!(
        src.contains("const _barsum="),
        "used arr builtin helper missing"
    );
    assert!(
        src.contains("_barsum(S.get('Bag__items'))"),
        "live arr builtin call missing"
    );
    assert!(
        !src.contains("_baruniq"),
        "unused arr builtin must be tree-shaken"
    );
    check_js_syntax(&js);

    fs::remove_dir_all(&work).ok();
}

/// Interpolation `{t()}` in `validate:*` values is resolved per-locale at build
/// (and string-literal escapes are undone).
#[test]
fn validate_attr_resolves_interpolation() {
    let work = scratch_dir("validate-i18n");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("locales")).expect("mkdir locales");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nlocale = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(work.join("locales/fr.toml"), "req = \"Nom requis\"\n").expect("write fr");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  form {\n    input type=\"text\" name=\"u\" validate:required=\"{t(\\\"req\\\")}\"\n  }\n}\n",
    )
    .expect("write page");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");
    assert!(
        html.contains(r#"data-webcore-validate-required="Nom requis""#),
        "validate message not resolved from t():\n{html}"
    );
    // The raw t() expression must not leak.
    assert!(
        !html.contains("t(\\\"req\\\")"),
        "raw t() expr leaked:\n{html}"
    );
    assert!(
        !html.contains("{t("),
        "unresolved interpolation leaked:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// After expanding a data collection: an `@if` whose condition became a literal
/// is folded (no runtime binding), and a nested `@for` over an item sub-array
/// is expanded statically.
#[test]
fn collection_folds_if_and_expands_nested_for() {
    let work = scratch_dir("collection-nested");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("data")).expect("mkdir data");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("data/projects.json"),
        "[{\"title\":\"Alpha\",\"featured\":true,\"tags\":[\"rust\",\"web\"]},{\"title\":\"Beta\",\"featured\":false,\"tags\":[\"cli\"]}]\n",
    )
    .expect("write data");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "import projects from \"data/projects.json\"\n\npage \"home\" {\n  @for project in projects {\n    article {\n      h3 \"{project.title}\"\n      @if project.featured { span class=\"star\" \"STAR\" }\n      ul { @for tag in project.tags { li \"{tag}\" } }\n    }\n  }\n}\n",
    )
    .expect("write page");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");

    assert!(
        html.contains("<h3>Alpha</h3>") && html.contains("<h3>Beta</h3>"),
        "titles missing:\n{html}"
    );
    // @if folded: STAR only for the featured item (Alpha), exactly once.
    assert_eq!(
        html.matches("STAR").count(),
        1,
        "@if not folded correctly:\n{html}"
    );
    // Nested @for over sub-array expanded statically.
    assert!(
        html.contains("<li>rust</li>")
            && html.contains("<li>web</li>")
            && html.contains("<li>cli</li>"),
        "tags not expanded:\n{html}"
    );
    // Fully static — no runtime bindings left.
    assert!(
        !html.contains("data-webcore-if"),
        "@if left as runtime binding:\n{html}"
    );
    assert!(
        !html.contains("data-webcore-for"),
        "nested @for left as runtime loop:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// An asset referenced only from a `data/*.json` collection field must not be
/// flagged orphan by `webc check` (the data dir is part of the haystack).
#[test]
fn check_orphan_ignores_data_referenced_assets() {
    let work = scratch_dir("orphan-data");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("public")).expect("mkdir public");
    fs::create_dir_all(work.join("data")).expect("mkdir data");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("data/projects.json"),
        "[{\"title\":\"A\",\"image\":\"/hero.png\"}]\n",
    )
    .expect("write data");
    fs::write(work.join("public/hero.png"), "x").expect("write hero");
    fs::write(work.join("public/orphan.png"), "x").expect("write orphan");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { h1 \"x\" }\n",
    )
    .expect("write page");

    let out = Command::new(webc_bin())
        .args(["check", "--json"])
        .current_dir(&work)
        .output()
        .expect("spawn webc check");
    let report: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).expect("valid JSON");
    let orphans: Vec<String> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "orphan-asset")
        .map(|d| d["message"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        !orphans.iter().any(|m| m.contains("hero.png")),
        "data-referenced asset wrongly flagged: {orphans:?}"
    );
    assert!(
        orphans.iter().any(|m| m.contains("orphan.png")),
        "true orphan should still be flagged: {orphans:?}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #1 regression — `webc check` must not flag a favicon declared in
/// `src/app.webc` nor a top-level `public/*.css` (auto-injected) as orphan.
#[test]
fn check_orphan_ignores_app_favicon_and_auto_css() {
    let work = scratch_dir("orphan-app");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("public")).expect("mkdir public");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("src/app.webc"),
        "app Portfolio {\n  layout: MainLayout\n  head { favicon \"/assets/favicon.svg\" }\n}\n",
    )
    .expect("write app");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { h1 \"x\" }\n",
    )
    .expect("write page");
    fs::write(work.join("public/favicon.svg"), "<svg/>").expect("write favicon");
    fs::write(work.join("public/styles.css"), "body{}").expect("write css");
    fs::write(work.join("public/orphan.svg"), "<svg/>").expect("write orphan");

    let out = Command::new(webc_bin())
        .args(["check", "--json"])
        .current_dir(&work)
        .output()
        .expect("spawn webc check");
    let report: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).expect("valid JSON");
    let orphans: Vec<String> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "orphan-asset")
        .map(|d| d["message"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        !orphans.iter().any(|m| m.contains("favicon.svg")),
        "app.webc favicon wrongly flagged: {orphans:?}"
    );
    assert!(
        !orphans.iter().any(|m| m.contains("styles.css")),
        "auto-injected css wrongly flagged: {orphans:?}"
    );
    assert!(
        orphans.iter().any(|m| m.contains("orphan.svg")),
        "true orphan should still be flagged: {orphans:?}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #3 — `meta key = "value"` accepts extra attributes (e.g. `media`), letting
/// two theme-color metas coexist for light/dark address bars.
#[test]
fn head_meta_extra_attributes() {
    let work = scratch_dir("meta-media");
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
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  head {\n    meta theme-color=\"#ffffff\" media=\"(prefers-color-scheme: light)\"\n    meta theme-color=\"#000000\" media=\"(prefers-color-scheme: dark)\"\n  }\n  h1 \"x\"\n}\n",
    )
    .expect("write page");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");
    assert!(
        html.contains(
            r##"<meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">"##
        ),
        "light theme-color meta missing:\n{html}"
    );
    assert!(
        html.contains(
            r##"<meta name="theme-color" content="#000000" media="(prefers-color-scheme: dark)">"##
        ),
        "dark theme-color meta missing:\n{html}"
    );

    fs::remove_dir_all(&work).ok();
}

/// #2 — jsonld supports nested objects and arrays (a full Schema.org Person).
#[test]
fn head_jsonld_nested_objects_and_arrays() {
    let work = scratch_dir("jsonld-nested");
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
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" {\n  head {\n    jsonld {\n      \"@type\": \"Person\"\n      name: \"Jonathan\"\n      address: {\n        \"@type\": \"PostalAddress\"\n        addressLocality: \"Bordeaux\"\n      }\n      sameAs: [\"https://github.com/x\", \"https://linkedin.com/x\"]\n      knowsAbout: [\"PHP\", \".NET\"]\n    }\n  }\n  h1 \"x\"\n}\n",
    )
    .expect("write page");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");
    let expected = r#"<script type="application/ld+json">{"@type":"Person","name":"Jonathan","address":{"@type":"PostalAddress","addressLocality":"Bordeaux"},"sameAs":["https://github.com/x","https://linkedin.com/x"],"knowsAbout":["PHP",".NET"]}</script>"#;
    assert!(html.contains(expected), "nested json-ld wrong:\n{html}");

    fs::remove_dir_all(&work).ok();
}

/// #4 — `{$build.*}` resolves to real build stats (page count, output sizes) as
/// static text, no runtime binding, no leftover placeholder.
#[test]
fn build_variables_resolve_to_static_stats() {
    let work = scratch_dir("build-vars");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } footer { p \"pages:{$build.pages} js:{$build.jsBytes}b\" } }\n",
    )
    .expect("write layout");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { h1 \"H\" }\n",
    )
    .expect("write home");
    fs::write(
        work.join("src/pages/about.webc"),
        "page \"about\" { h1 \"A\" }\n",
    )
    .expect("write about");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");

    // Text/interpolation segments are newline-separated in the HTML (rendered
    // inline); compare on a whitespace-stripped copy.
    let compact: String = html.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.contains("pages:2js:"),
        "page count not resolved:\n{html}"
    );
    assert!(
        regex_lite_digits_after(&compact, "js:"),
        "jsBytes not a number:\n{html}"
    );
    assert!(
        !html.contains("wcbuild:"),
        "build placeholder leaked:\n{html}"
    );
    assert!(
        !html.contains('\u{2063}'),
        "invisible marker leaked:\n{html}"
    );
    assert!(!html.contains("$build."), "raw $build expr leaked:\n{html}");

    fs::remove_dir_all(&work).ok();
}

/// Tiny helper: true if `needle` is immediately followed by an ASCII digit.
fn regex_lite_digits_after(haystack: &str, needle: &str) -> bool {
    haystack
        .split(needle)
        .skip(1)
        .any(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()))
}

/// #5 — `[app] view_transitions = true` wraps SPA navigation in
/// document.startViewTransition() (with fallback); off by default.
#[test]
fn view_transitions_opt_in() {
    let build = |enabled: bool| -> String {
        let work = scratch_dir(if enabled { "vt-on" } else { "vt-off" });
        fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
        fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
        let vt = if enabled {
            "view_transitions = true\n"
        } else {
            ""
        };
        fs::write(
            work.join("webc.toml"),
            format!("[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n{vt}"),
        )
        .expect("write webc.toml");
        fs::write(
            work.join("src/app.webc"),
            "app A {\n  layout: MainLayout\n  routes { \"/\": HomePage \"/about\": AboutPage }\n}\n",
        )
        .expect("write app");
        fs::write(
            work.join("src/layouts/MainLayout.webc"),
            "layout MainLayout { nav { link to=\"/about\" { \"About\" } } main { slot content } }\n",
        )
        .expect("write layout");
        fs::write(
            work.join("src/pages/home.webc"),
            "page \"home\" { h1 \"H\" }\n",
        )
        .expect("write home");
        fs::write(
            work.join("src/pages/about.webc"),
            "page \"about\" { h1 \"A\" }\n",
        )
        .expect("write about");

        run_build(&work);
        let js = find_runtime_js(&work.join("dist")).expect("runtime js");
        check_js_syntax(&js);
        let src = fs::read_to_string(&js).expect("read js");
        fs::remove_dir_all(&work).ok();
        src
    };

    let on = build(true);
    assert!(
        on.contains("document.startViewTransition(apply)"),
        "view transition wrap missing when enabled"
    );
    assert!(on.contains("else apply()"), "fallback missing when enabled");

    let off = build(false);
    assert!(
        !off.contains("startViewTransition"),
        "view transition must be absent when disabled"
    );
    assert!(off.contains("apply()"), "nav apply() missing when disabled");
}

/// Gzipped build variables resolve to numbers (real over-the-wire size).
#[test]
fn build_variables_gzip_sizes() {
    let work = scratch_dir("build-gzip");
    fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
    fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
    fs::create_dir_all(work.join("src/components")).expect("mkdir comps");
    fs::write(
        work.join("webc.toml"),
        "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"prod\"\n",
    )
    .expect("write webc.toml");
    fs::write(
        work.join("src/layouts/MainLayout.webc"),
        "layout MainLayout { main { slot content } footer { p \"js:{$build.jsGzipBytes} total:{$build.totalGzipKb}kB\" } }\n",
    )
    .expect("write layout");
    // A component with state so a real runtime is emitted (non-trivial gzip).
    fs::write(
        work.join("src/components/Counter.webc"),
        "component Counter { state { n: Number = 0 } view { button on:click={n += 1} { \"{n}\" } } }\n",
    )
    .expect("write comp");
    fs::write(
        work.join("src/pages/home.webc"),
        "page \"home\" { Counter {} }\n",
    )
    .expect("write page");

    run_build(&work);
    let html = fs::read_to_string(work.join("dist/index.html")).expect("index");
    let compact: String = html.chars().filter(|c| !c.is_whitespace()).collect();
    // Both gzip vars resolved to a digit; no placeholder left.
    assert!(
        compact
            .split("js:")
            .nth(1)
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit())),
        "jsGzipBytes not resolved:\n{html}"
    );
    assert!(compact.contains("total:"), "totalGzipKb missing:\n{html}");
    assert!(!html.contains("wcbuild:"), "placeholder leaked:\n{html}");

    fs::remove_dir_all(&work).ok();
}

/// The PWA theme-color is suppressed when the head declares its own (so
/// light/dark `media` variants aren't clobbered); still emitted otherwise.
#[test]
fn pwa_theme_color_yields_to_head() {
    let build = |head_theme: bool| -> String {
        let work = scratch_dir(if head_theme { "pwa-head" } else { "pwa-nohead" });
        fs::create_dir_all(work.join("src/layouts")).expect("mkdir layouts");
        fs::create_dir_all(work.join("src/pages")).expect("mkdir pages");
        fs::write(
            work.join("webc.toml"),
            "[app]\ntitle = \"T\"\nlang = \"fr\"\nmode = \"dev\"\n\n[pwa]\nname = \"App\"\ntheme_color = \"#123456\"\n",
        )
        .expect("write webc.toml");
        fs::write(
            work.join("src/layouts/MainLayout.webc"),
            "layout MainLayout { main { slot content } }\n",
        )
        .expect("write layout");
        let head = if head_theme {
            "  head {\n    meta theme-color=\"#ffffff\" media=\"(prefers-color-scheme: light)\"\n    meta theme-color=\"#000000\" media=\"(prefers-color-scheme: dark)\"\n  }\n"
        } else {
            ""
        };
        fs::write(
            work.join("src/pages/home.webc"),
            format!("page \"home\" {{\n{head}  h1 \"x\"\n}}\n"),
        )
        .expect("write page");
        run_build(&work);
        let html = fs::read_to_string(work.join("dist/index.html")).expect("index");
        fs::remove_dir_all(&work).ok();
        html
    };

    // With head theme-color: no bare PWA theme-color; head variants present; PWA icon still there.
    let with = build(true);
    assert!(
        !with.contains(r##"<meta name="theme-color" content="#123456">"##),
        "PWA theme-color must yield to the head's:\n{with}"
    );
    assert!(
        with.contains("media=\"(prefers-color-scheme: dark)\""),
        "head variant missing"
    );
    assert!(
        with.contains("apple-touch-icon"),
        "other PWA tags must remain"
    );

    // Without a head theme-color: PWA still emits its own.
    let without = build(false);
    assert!(
        without.contains(r##"<meta name="theme-color" content="#123456">"##),
        "PWA theme-color should be emitted when head has none:\n{without}"
    );
}
