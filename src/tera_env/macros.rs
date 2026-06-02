//! Auto-import preamble for `templates/macros/*.html` (Phase 11). Used by the
//! markup env so Markdown bodies can invoke macros as shortcodes without
//! writing `{% import %}` boilerplate.

use crate::config::Config;
use std::fs;

/// Scan `macros/*.html` (non-recursive) under every `config.template_roots()`
/// and build a Tera import preamble: one `{% import "macros/<stem>.html" as
/// <stem> %}` per unique macro stem. A stem present in both theme and site yields
/// a single import — `load_templates` overlays the site's file under that name,
/// so the import resolves to the site's macro. Sorted alphabetically by stem so
/// output is deterministic across platforms (the FS read order is not). Returns
/// `""` when no root has a `macros/` dir with `.html` files.
pub fn discover_imports(config: &Config) -> String {
    let mut stems: Vec<String> = Vec::new();
    for root in config.template_roots() {
        let macros_dir = root.join("macros");
        let Ok(entries) = fs::read_dir(&macros_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("html") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let stem = stem.to_string();
                if !stems.contains(&stem) {
                    stems.push(stem);
                }
            }
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
