use crate::doc::Doc;
use crate::frontmatter;
use crate::query::Query;
use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};
use std::path::PathBuf;

/// One file in `generators/`. Frontmatter declares the query and the
/// per-page pattern; the body is the Tera template that each emitted page
/// renders. See spec §6 (phase 3) and §9.1.
pub struct Generator {
    pub id_path: PathBuf,
    pub query: Query,
    pub per_page: Option<usize>,
    pub permalink: String,
    pub template: Option<String>,
    /// Ordering key for generator execution (lower runs first). Named
    /// `weight` rather than spec.md's `order` to disambiguate from the
    /// `query.order_by` field.
    pub weight: i32,
    pub body: String,
    /// Verbatim frontmatter, so generator bodies can refer to any author-
    /// supplied fields via `{{ doc.data.xxx }}`.
    pub data: Mapping,
}

/// Pagination context for a single emitted page. Serialized into the
/// generator-emitted `Doc`'s `data.pagination` and surfaced at the top
/// level of the Tera context (see `markup::render` and `template::run`).
#[derive(Serialize)]
pub struct Pagination {
    pub current: usize,
    pub total: usize,
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
    pub items: Vec<Doc>,
}

impl Generator {
    /// Parse a generator file. `permalink` is required; everything else is
    /// optional with sensible defaults.
    pub fn parse(id_path: PathBuf, source: &str) -> Result<Generator> {
        let (data, body) = frontmatter::parse(source)?;

        let permalink = data
            .get("permalink")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow!(
                    "generator `{}` missing required `permalink` field",
                    id_path.display()
                )
            })?
            .to_string();

        let per_page = data
            .get("per_page")
            .and_then(Value::as_u64)
            .map(|n| n as usize);

        let template = data
            .get("template")
            .and_then(Value::as_str)
            .map(str::to_string);

        let weight = data
            .get("weight")
            .and_then(Value::as_i64)
            .map(|n| n as i32)
            .unwrap_or(0);

        let query = match data.get("query") {
            Some(Value::Mapping(m)) => Query::from_yaml_mapping(m)?,
            Some(_) => return Err(anyhow!("`query` must be a YAML mapping")),
            None => Query::default(),
        };

        Ok(Generator {
            id_path,
            query,
            per_page,
            permalink,
            template,
            weight,
            body: body.to_string(),
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_generator() {
        let source = "---\n\
permalink: /blog/page-:page/\n\
per_page: 2\n\
weight: 5\n\
template: blog.html\n\
query:\n  path: \"posts/*.md\"\n  order_by: date\n  sort: desc\n\
---\nBODY";
        let g = Generator::parse(PathBuf::from("blog.html"), source).unwrap();
        assert_eq!(g.permalink, "/blog/page-:page/");
        assert_eq!(g.per_page, Some(2));
        assert_eq!(g.weight, 5);
        assert_eq!(g.template.as_deref(), Some("blog.html"));
        assert_eq!(g.body, "BODY");
        assert!(g.query.path.is_some());
    }

    #[test]
    fn parse_defaults() {
        let source = "---\npermalink: /sitemap.xml\n---\nBODY";
        let g = Generator::parse(PathBuf::from("sitemap.xml"), source).unwrap();
        assert_eq!(g.permalink, "/sitemap.xml");
        assert_eq!(g.per_page, None);
        assert_eq!(g.weight, 0);
        assert!(g.template.is_none());
        // No `query:` → default Query (matches everything).
        assert!(g.query.path.is_none());
        assert!(g.query.tag.is_none());
    }

    #[test]
    fn parse_missing_permalink_errors() {
        let source = "---\nweight: 1\n---\nBODY";
        assert!(Generator::parse(PathBuf::from("x.html"), source).is_err());
    }
}
