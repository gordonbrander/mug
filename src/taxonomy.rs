//! Taxonomies: named classifications of docs (Hugo's model — a taxonomy has
//! terms, each term groups docs). A taxonomy is just a string: the frontmatter
//! field a doc uses to declare its terms, which is also the taxonomy's name.
//! Taxonomies are declared as an array under the `taxonomies:` key in
//! `config.yaml`; there are no built-in defaults (the scaffolded `config.yaml`
//! declares `tags` like any other). Each doc carries its term memberships in
//! `Doc.terms` (taxonomy name → slug → display text); the index inverts those
//! into `taxonomy → term → docs` for the `taxonomy()` template function and the
//! taxonomy archives.

use anyhow::{Result, anyhow};
use serde_yaml_ng::Sequence;

/// The taxonomy name that inline `#hashtags` are bucketed under during the
/// markup phase (see `build::markup`). It is not auto-registered as a taxonomy —
/// hashtags only become browsable when the user declares `tags` under
/// `taxonomies:` in `config.yaml`.
pub const BUILTIN: &str = "tags";

/// Parse the `taxonomies:` array into the declared taxonomy names, in
/// declaration order. Each entry must be a string; duplicates are skipped so a
/// repeated name doesn't create two identical buckets. `None` (no `taxonomies:`
/// key) yields an empty list — there are no built-in taxonomies.
pub fn parse(seq: Option<&Sequence>) -> Result<Vec<String>> {
    let mut taxonomies: Vec<String> = Vec::new();
    let Some(seq) = seq else {
        return Ok(taxonomies);
    };
    for value in seq {
        let name = value
            .as_str()
            .ok_or_else(|| anyhow!("taxonomies: entries must be strings"))?;
        if !taxonomies.iter().any(|t| t == name) {
            taxonomies.push(name.to_string());
        }
    }
    Ok(taxonomies)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(s: &str) -> Sequence {
        serde_yaml_ng::from_str(s).unwrap()
    }

    #[test]
    fn none_yields_empty() {
        let t = parse(None).unwrap();
        assert!(t.is_empty());
    }

    #[test]
    fn parses_in_declaration_order() {
        let s = yaml("- tags\n- categories\n- series\n");
        let t = parse(Some(&s)).unwrap();
        assert_eq!(t, vec!["tags", "categories", "series"]);
    }

    #[test]
    fn duplicates_are_skipped() {
        let s = yaml("- tags\n- categories\n- tags\n");
        let t = parse(Some(&s)).unwrap();
        assert_eq!(t, vec!["tags", "categories"]);
    }

    #[test]
    fn non_string_entry_errors() {
        let s = yaml("- tags\n- 7\n");
        assert!(parse(Some(&s)).is_err());
    }
}
