//! CSS Post-processing with LightningCSS

use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::{Browsers, Targets};

pub fn process_css(css: &str, minify: bool) -> Result<String, String> {
    // Parse CSS
    let stylesheet = StyleSheet::parse(
        css,
        ParserOptions {
            ..Default::default()
        },
    )
    .map_err(|e| format!("Failed to parse CSS: {}", e))?;

    // Configure targets for modern browsers
    let targets = Targets {
        browsers: Some(Browsers {
            chrome: Some(90),
            firefox: Some(88),
            safari: Some(14),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Configure printer options
    let printer_options = PrinterOptions {
        minify,
        targets,
        ..Default::default()
    };

    // Generate optimized CSS
    let result = stylesheet
        .to_css(printer_options)
        .map_err(|e| format!("Failed to generate CSS: {}", e))?;

    Ok(result.code)
}

pub fn minify_css(css: &str) -> Result<String, String> {
    process_css(css, true)
}

pub fn format_css(css: &str) -> Result<String, String> {
    process_css(css, false)
}
