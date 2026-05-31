//! Taxonomies: named classifications of docs (Hugo's model — a taxonomy has
//! terms, each term groups docs). Taxonomies are declared under the
//! `taxonomies:` key in `config.yaml`; there are no built-in defaults (the
//! scaffolded `config.yaml` declares `tags` like any other). Each doc carries
//! its term memberships in `Doc.terms` (taxonomy name → slug → display text);
//! the index inverts those into `taxonomy → term → docs` for the `taxonomy()`
//! template function and the taxonomy archives.

use anyhow::{Result, anyhow};
use serde_yaml_ng::{Mapping, Value};

/// The taxonomy name that inline `#hashtags` are bucketed under during the
/// markup phase (see `build::markup`). It is not auto-registered as a taxonomy —
/// hashtags only become browsable when the user declares `tags` under
/// `taxonomies:` in `config.yaml`.
pub const BUILTIN: &str = "tags";

/// A single taxonomy. `field` is the frontmatter key a doc uses to declare its
/// terms; it defaults to `name` (so `categories:` in frontmatter feeds the
/// `categories` taxonomy) but can be overridden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Taxonomy {
    pub name: String,
    pub field: String,
}

impl Taxonomy {
    /// A taxonomy whose frontmatter field matches its name.
    pub fn new(name: impl Into<String>) -> Taxonomy {
        let name = name.into();
        let field = name.clone();
        Taxonomy { name, field }
    }
}

/// Parse the `taxonomies:` mapping into the declared taxonomies, in declaration
/// order.
///
/// Each entry is `name: <options>` where options is a mapping (currently only
/// `field:`), `null`/`true` (field defaults to name), or `false` to remove a
/// previously-declared taxonomy of that name. `None` (no `taxonomies:` key)
/// yields an empty list — there are no built-in taxonomies.
pub fn parse(map: Option<&Mapping>) -> Result<Vec<Taxonomy>> {
    let mut taxonomies: Vec<Taxonomy> = Vec::new();
    let Some(map) = map else {
        return Ok(taxonomies);
    };
    for (key, value) in map {
        let name = key
            .as_str()
            .ok_or_else(|| anyhow!("taxonomies: keys must be strings"))?;
        // `name: false` removes the taxonomy (e.g. opt out of built-in tags).
        if matches!(value, Value::Bool(false)) {
            taxonomies.retain(|t| t.name != name);
            continue;
        }
        let field = match value {
            Value::Null | Value::Bool(true) => name.to_string(),
            Value::Mapping(m) => match m.get("field") {
                Some(v) => v
                    .as_str()
                    .ok_or_else(|| {
                        anyhow!("taxonomies: `field` for `{}` must be a string", name)
                    })?
                    .to_string(),
                None => name.to_string(),
            },
            _ => {
                return Err(anyhow!(
                    "taxonomies: value for `{}` must be a mapping, null, or false",
                    name
                ));
            }
        };
        match taxonomies.iter_mut().find(|t| t.name == name) {
            Some(existing) => existing.field = field,
            None => taxonomies.push(Taxonomy {
                name: name.to_string(),
                field,
            }),
        }
    }
    Ok(taxonomies)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(s: &str) -> Mapping {
        serde_yaml_ng::from_str(s).unwrap()
    }

    fn names(t: &[Taxonomy]) -> Vec<&str> {
        t.iter().map(|t| t.name.as_str()).collect()
    }

    #[test]
    fn none_yields_empty() {
        let t = parse(None).unwrap();
        assert!(t.is_empty());
    }

    #[test]
    fn parses_in_declaration_order() {
        let m = yaml("tags:\ncategories:\nseries:\n");
        let t = parse(Some(&m)).unwrap();
        // No built-in: order is exactly the declaration order.
        assert_eq!(names(&t), vec!["tags", "categories", "series"]);
    }

    #[test]
    fn field_defaults_to_name() {
        let m = yaml("categories:\n");
        let t = parse(Some(&m)).unwrap();
        assert_eq!(t[0].name, "categories");
        assert_eq!(t[0].field, "categories");
    }

    #[test]
    fn field_can_be_overridden() {
        let m = yaml("series:\n  field: serie\n");
        let t = parse(Some(&m)).unwrap();
        assert_eq!(t[0].name, "series");
        assert_eq!(t[0].field, "serie");
    }

    #[test]
    fn false_removes_a_declared_taxonomy() {
        // `false` removes an earlier declaration; a lone `false` is a no-op.
        let m = yaml("tags: false\ncategories:\n");
        let t = parse(Some(&m)).unwrap();
        assert_eq!(names(&t), vec!["categories"]);
    }

    #[test]
    fn tags_field_can_be_overridden() {
        let m = yaml("tags:\n  field: topics\n");
        let t = parse(Some(&m)).unwrap();
        assert_eq!(names(&t), vec!["tags"]);
        assert_eq!(t[0].field, "topics");
    }

    #[test]
    fn non_string_field_errors() {
        let m = yaml("series:\n  field: 7\n");
        assert!(parse(Some(&m)).is_err());
    }

    #[test]
    fn scalar_value_errors() {
        let m = yaml("categories: 7\n");
        assert!(parse(Some(&m)).is_err());
    }
}
