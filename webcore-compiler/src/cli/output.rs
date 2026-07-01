//! Build output formatting: dist tree and bundle analysis.

use std::fs;
use std::path::Path;

fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} kB", bytes as f64 / 1024.0)
    }
}

pub(crate) fn fmt_bytes(b: u64) -> String {
    if b >= 1024 {
        format!("{:.1} kB", b as f64 / 1024.0)
    } else {
        format!("{b} B")
    }
}

pub(crate) fn print_dist_tree(dist_dir: &Path, minified: bool) {
    // Collect all files recursively
    let mut files: Vec<(String, u64)> = Vec::new();
    fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, u64)>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let rel = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}/{name}")
                };
                if path.is_dir() {
                    collect(&path, &rel, out);
                } else if let Ok(meta) = fs::metadata(&path) {
                    out.push((rel, meta.len()));
                }
            }
        }
    }
    collect(dist_dir, "", &mut files);

    // Sort: html first (alpha), then js, then css, then rest
    files.sort_by(|(a, _), (b, _)| {
        fn rank(s: &str) -> u8 {
            if s.ends_with(".html") {
                0
            } else if s.ends_with(".js") {
                1
            } else if s.ends_with(".css") {
                2
            } else {
                3
            }
        }
        rank(a).cmp(&rank(b)).then(a.cmp(b))
    });

    let total_bytes: u64 = files.iter().map(|(_, s)| s).sum();
    let max_name = files.iter().map(|(n, _)| n.len()).max().unwrap_or(10);

    println!("\ndist/");
    let count = files.len();
    for (i, (name, size)) in files.iter().enumerate() {
        let branch = if i + 1 == count {
            "└──"
        } else {
            "├──"
        };
        println!(
            "  {}  {:<width$}  {}",
            branch,
            name,
            fmt_size(*size),
            width = max_name
        );
    }
    let mode_label = if minified { "minified" } else { "dev" };
    println!(
        "\n  {} file{}  {}  ({})\n",
        count,
        if count == 1 { "" } else { "s" },
        fmt_size(total_bytes),
        mode_label,
    );
}

/// Print a bundle analysis table showing which runtime features were included
/// or tree-shaken, along with estimated byte contributions.
pub(crate) fn print_bundle_analysis(js: &str) {
    /// (marker_string, human_label, estimated_bytes)
    const FEATURES: &[(&str, &str, usize)] = &[
        ("bindIf", "bindIf (conditionals)", 320),
        ("bindFor", "bindFor (loops)", 512),
        ("bindAttrs", "bindAttrs (dyn attrs)", 180),
        ("bindClassBindings", "bindClassBindings (class:)", 140),
        ("bindValidation", "bindValidation", 640),
        ("const LOCALES=", "i18n / t()", 210),
        ("const ROUTES=", "router (param routes)", 380),
        ("const toFile=", "router (simple nav)", 90),
        ("const WASM=", "WASM loader", 120),
        ("DESTROY_HOOKS", "on:destroy hooks", 80),
        ("const COMPUTED=", "computed vars", 95),
    ];

    // Rough estimate: state-class boilerplate ~420 bytes + ~350 per reactive component.
    let core_bytes = js.matches("class State{").count() * 350 + 420;

    let mut total: u64 = core_bytes as u64;
    println!("\n  Bundle analysis");
    println!("  ──────────────────────────────────────────────");
    println!("  {:<35} {:<10} Status", "Feature", "Size");
    println!("  {:<35} {:<10} ──────", "───────", "────");
    println!(
        "  {:<35} {:<10} ✓ included",
        "runtime core",
        fmt_bytes(core_bytes as u64)
    );

    for &(marker, label, est) in FEATURES {
        let included = js.contains(marker);
        if included {
            total += est as u64;
        }
        let status = if included {
            "✓ included"
        } else {
            "- tree-shaken"
        };
        let size_str = if included {
            fmt_bytes(est as u64)
        } else {
            "   —".to_string()
        };
        println!("  {:<35} {:<10} {}", label, size_str, status);
    }

    println!("  {}", "─".repeat(54));
    println!(
        "  {:<35} {:<10}",
        "estimated total (unminified)",
        fmt_bytes(total)
    );
    println!();
}
