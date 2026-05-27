//! Auto-import preamble for `templates/macros/*.html` (Phase 11). Used by the
//! markup env so Markdown bodies can invoke macros as shortcodes without
//! writing `{% import %}` boilerplate.

use crate::config::Config;
use std::fs;

/// Scan `templates/macros/*.html` (non-recursive) and build a Tera import
/// preamble: one `{% import "macros/<stem>.html" as <stem> %}` per macro file.
/// Sorted alphabetically by stem so output is deterministic across platforms
/// (the FS read order is not). Returns `""` if `templates/` or
/// `templates/macros/` is missing, or the dir contains no `.html` files.
pub fn discover_imports(config: &Config) -> String {
    let macros_dir = config.templates_dir.join("macros");
    let Ok(entries) = fs::read_dir(&macros_dir) else {
        return String::new();
    };

    let mut stems: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("html") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            stems.push(stem.to_string());
        }
    }
    stems.sort();

    let mut out = String::new();
    for stem in stems {
        out.push_str(&format!(
            "{{% import \"macros/{stem}.html\" as {stem} %}}\n"
        ));
    }
    out
}
