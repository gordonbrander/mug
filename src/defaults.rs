use anyhow::{anyhow, Result};
use globset::{GlobBuilder, GlobMatcher};
use serde_yaml_ng::{Mapping, Value};
use std::path::Path;

/// Glob-scoped default frontmatter from `config.yaml`'s `defaults:` key. Each
/// entry maps a glob to a map of frontmatter values that fill keys a document
/// did not set itself. Applied during the read phase, before `Doc::new`
/// uplifts fields and computes `output_path`, so `permalink` (and anything
/// else) can be defaulted by file pattern.
#[derive(Debug, Default)]
pub struct Defaults {
    /// (compiled glob matcher, default frontmatter map), in config order.
    /// Later entries win. Matchers are compiled once at construction.
    rules: Vec<(GlobMatcher, Mapping)>,
}

impl Defaults {
    /// Parse the `defaults:` sub-mapping: each key is a glob string, each
    /// value a map of default frontmatter values. Globs use the same
    /// `literal_separator(true)` convention as `query` (see `query.rs`), so
    /// `*` does not cross `/`.
    pub fn from_yaml_mapping(m: &Mapping) -> Result<Self> {
        let mut rules = Vec::new();
        for (k, v) in m {
            let pat = k
                .as_str()
                .ok_or_else(|| anyhow!("defaults: keys must be glob strings"))?;
            let matcher = GlobBuilder::new(pat)
                .literal_separator(true)
                .build()
                .map_err(|e| anyhow!("defaults: invalid glob `{}`: {}", pat, e))?
                .compile_matcher();
            let Value::Mapping(map) = v else {
                return Err(anyhow!("defaults: value for `{}` must be a mapping", pat));
            };
            rules.push((matcher, map.clone()));
        }
        Ok(Self { rules })
    }

    /// Fill keys absent from `data` using every rule whose glob matches
    /// `id_path`. Later matching rules win among defaults; `data` (the
    /// document's own frontmatter) always wins. Applied to the raw
    /// frontmatter map *before* `Doc::new` uplifts fields, so no rebuild is
    /// needed. Returns whether any default was applied.
    pub fn apply(&self, id_path: &Path, data: &mut Mapping) -> bool {
        // Combine matching rules in order so later entries overwrite earlier.
        let mut combined = Mapping::new();
        for (matcher, map) in &self.rules {
            if matcher.is_match(id_path) {
                for (k, v) in map {
                    combined.insert(k.clone(), v.clone());
                }
            }
        }
        let mut changed = false;
        for (k, v) in combined {
            if !data.contains_key(&k) {
                data.insert(k, v);
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn yaml_mapping(s: &str) -> Mapping {
        serde_yaml_ng::from_str(s).unwrap()
    }

    fn defaults(s: &str) -> Defaults {
        Defaults::from_yaml_mapping(&yaml_mapping(s)).unwrap()
    }

    #[test]
    fn empty_defaults_apply_nothing() {
        let d = Defaults::default();
        let mut data = Mapping::new();
        assert!(!d.apply(Path::new("posts/a.md"), &mut data));
        assert!(data.is_empty());
    }

    #[test]
    fn from_yaml_mapping_rejects_invalid_glob() {
        let m = yaml_mapping("\"posts/[.md\":\n  permalink: x\n");
        assert!(Defaults::from_yaml_mapping(&m).is_err());
    }

    #[test]
    fn from_yaml_mapping_rejects_non_mapping_value() {
        let m = yaml_mapping("\"posts/*.md\": not-a-map\n");
        assert!(Defaults::from_yaml_mapping(&m).is_err());
    }

    #[test]
    fn apply_fills_missing_key() {
        let d = defaults("\"posts/*.md\":\n  permalink: \":yyyy/:mm/:dd/:slug\"\n");
        let mut data = Mapping::new();
        assert!(d.apply(Path::new("posts/hello.md"), &mut data));
        assert_eq!(
            data.get("permalink").and_then(Value::as_str),
            Some(":yyyy/:mm/:dd/:slug")
        );
    }

    #[test]
    fn document_frontmatter_wins_over_default() {
        let d = defaults("\"posts/*.md\":\n  permalink: \":slug\"\n");
        let mut data = yaml_mapping("permalink: /custom/\n");
        // A matching default exists, but the document already set the key, so
        // nothing changes.
        assert!(!d.apply(Path::new("posts/hello.md"), &mut data));
        assert_eq!(
            data.get("permalink").and_then(Value::as_str),
            Some("/custom/")
        );
    }

    #[test]
    fn last_matching_rule_wins_among_defaults() {
        // Both globs match; the later one should win for `permalink`.
        let d = defaults(
            "\"posts/*.md\":\n  permalink: first\n\"posts/special.md\":\n  permalink: second\n",
        );
        let mut data = Mapping::new();
        assert!(d.apply(Path::new("posts/special.md"), &mut data));
        assert_eq!(
            data.get("permalink").and_then(Value::as_str),
            Some("second")
        );
    }

    #[test]
    fn non_matching_glob_inserts_nothing() {
        let d = defaults("\"posts/*.md\":\n  permalink: x\n");
        let mut data = Mapping::new();
        assert!(!d.apply(Path::new("pages/about.md"), &mut data));
        assert!(data.is_empty());
    }

    #[test]
    fn literal_separator_does_not_cross_slash() {
        let d = defaults("\"posts/*.md\":\n  permalink: x\n");
        let mut shallow = Mapping::new();
        assert!(d.apply(Path::new("posts/a.md"), &mut shallow));
        let mut nested = Mapping::new();
        assert!(!d.apply(Path::new("posts/sub/b.md"), &mut nested));
    }
}
