//! Auto-import preamble for `templates/macros/*.html` (Phase 11). Used by the
//! markup env so Markdown bodies can invoke macros as shortcodes without
//! writing `{% import %}` boilerplate.

/// Build a Tera import preamble from the loaded template `names`: one
/// `{% import "macros/<stem>.html" as <stem> %}` per macro file. A *macro file*
/// is a top-level entry under `macros/` — a name of the form `macros/<stem>.html`
/// with exactly one `/` (nested `macros/sub/x.html` are not macros, matching the
/// historical non-recursive scan). Because `names` come from `load_templates`
/// already deduped in overlay order (site wins), a stem present in both theme and
/// site appears once and its import resolves to the site's file. Sorted
/// alphabetically by stem so output is deterministic. Returns `""` when there are
/// no macro files.
pub fn macro_preamble(names: &[String]) -> String {
    let mut stems: Vec<&str> = names
        .iter()
        .filter_map(|name| macro_stem(name))
        .collect();
    stems.sort_unstable();
    stems.dedup();

    let mut out = String::new();
    for stem in stems {
        out.push_str(&format!("{{% import \"macros/{stem}.html\" as {stem} %}}\n"));
    }
    out
}

/// The macro stem of a template name, if it is a top-level `macros/<stem>.html`
/// (exactly one path segment under `macros/`). Returns `None` otherwise.
fn macro_stem(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("macros/")?;
    let stem = rest.strip_suffix(".html")?;
    // Reject nested paths like `macros/sub/x.html` — macros are flat.
    if stem.is_empty() || stem.contains('/') {
        return None;
    }
    Some(stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_only_flat_macro_files_sorted_and_deduped() {
        let names = vec![
            "post.html".to_string(),
            "macros/youtube.html".to_string(),
            "macros/note.html".to_string(),
            "macros/sub/nested.html".to_string(), // nested → not a macro
            "macros/youtube.html".to_string(),    // duplicate → one import
            "base.html".to_string(),
        ];
        assert_eq!(
            macro_preamble(&names),
            "{% import \"macros/note.html\" as note %}\n\
             {% import \"macros/youtube.html\" as youtube %}\n"
        );
    }

    #[test]
    fn empty_when_no_macro_files() {
        let names = vec!["post.html".to_string(), "base.html".to_string()];
        assert_eq!(macro_preamble(&names), "");
    }
}
