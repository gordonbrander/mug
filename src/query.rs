use crate::doc::Doc;
use anyhow::{Result, anyhow};
use globset::{Glob, GlobBuilder, GlobMatcher};
use serde_yaml_ng::{Mapping, Value as YamlValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderKey {
    Title,
    Created,
    Updated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

pub struct Query {
    pub path: Option<Glob>,
    pub tag: Option<String>,
    pub order_by: OrderKey,
    pub sort: SortDir,
    pub limit: Option<usize>,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            path: None,
            tag: None,
            order_by: OrderKey::Created,
            sort: SortDir::Desc,
            limit: None,
        }
    }
}

/// Field names accepted by query-style kwargs. Shared with the Tera adapter
/// in `tera_env::query` so both parsers reject the same typos.
pub(crate) const KNOWN_KEYS: &[&str] = &["path", "tag", "order_by", "sort", "limit"];

impl Query {
    /// Build from a YAML map (e.g. the `query:` sub-mapping in a generator's
    /// frontmatter). Matching kwargs parsing for the Tera `query()` function
    /// lives in `tera_env::query` — kept as a sibling rather than collapsed
    /// into a single generic, because serde_yaml_ng::Value and tera::Value
    /// have different shapes and the parsing is small enough that abstraction
    /// would cost more than it saves.
    pub fn from_yaml_mapping(m: &Mapping) -> Result<Self> {
        for k in m.keys() {
            let key = k
                .as_str()
                .ok_or_else(|| anyhow!("query: keys must be strings"))?;
            if !KNOWN_KEYS.contains(&key) {
                return Err(anyhow!(
                    "query: unknown key `{}` (allowed: {})",
                    key,
                    KNOWN_KEYS.join(", ")
                ));
            }
        }

        let mut q = Self::default();

        if let Some(v) = m.get("path") {
            let s = v
                .as_str()
                .ok_or_else(|| anyhow!("query: `path` must be a string"))?;
            let glob = GlobBuilder::new(s)
                .literal_separator(true)
                .build()
                .map_err(|e| anyhow!("query: invalid glob `{}`: {}", s, e))?;
            q.path = Some(glob);
        }

        if let Some(v) = m.get("tag") {
            let s = v
                .as_str()
                .ok_or_else(|| anyhow!("query: `tag` must be a string"))?;
            q.tag = Some(s.to_string());
        }

        if let Some(v) = m.get("order_by") {
            let s = v
                .as_str()
                .ok_or_else(|| anyhow!("query: `order_by` must be a string"))?;
            q.order_by = match s {
                "title" => OrderKey::Title,
                "created" => OrderKey::Created,
                "updated" => OrderKey::Updated,
                other => {
                    return Err(anyhow!(
                        "query: `order_by` must be one of title|created|updated (got `{}`)",
                        other
                    ));
                }
            };
        }

        if let Some(v) = m.get("sort") {
            let s = v
                .as_str()
                .ok_or_else(|| anyhow!("query: `sort` must be a string"))?;
            q.sort = match s {
                "asc" => SortDir::Asc,
                "desc" => SortDir::Desc,
                other => {
                    return Err(anyhow!(
                        "query: `sort` must be one of asc|desc (got `{}`)",
                        other
                    ));
                }
            };
        }

        if let Some(v) = m.get("limit") {
            let n = match v {
                YamlValue::Number(n) => n
                    .as_u64()
                    .ok_or_else(|| anyhow!("query: `limit` must be a non-negative integer"))?,
                _ => return Err(anyhow!("query: `limit` must be an integer")),
            };
            q.limit = Some(n as usize);
        }

        Ok(q)
    }
}

/// Linear scan over `docs`: filter, sort, then truncate. No secondary indexes —
/// the spec §11 "fully populated before listing" invariant is what makes this
/// correct, not the data structure.
pub fn evaluate<'a>(q: &Query, docs: &'a [Doc]) -> Vec<&'a Doc> {
    let matcher: Option<GlobMatcher> = q.path.as_ref().map(Glob::compile_matcher);

    let mut results: Vec<&Doc> = docs
        .iter()
        .filter(|d| {
            if let Some(m) = &matcher {
                if !m.is_match(&d.id_path) {
                    return false;
                }
            }
            if let Some(tag) = &q.tag {
                if !d.tags.iter().any(|t| t == tag) {
                    return false;
                }
            }
            true
        })
        .collect();

    results.sort_by(|a, b| {
        let cmp = match q.order_by {
            OrderKey::Title => a.title.cmp(&b.title),
            OrderKey::Created => a.date.cmp(&b.date),
            OrderKey::Updated => a.updated.cmp(&b.updated),
        };
        match q.sort {
            SortDir::Asc => cmp,
            SortDir::Desc => cmp.reverse(),
        }
    });

    if let Some(n) = q.limit {
        results.truncate(n);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};
    use std::path::PathBuf;

    fn at(date: &str) -> DateTime<Utc> {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
    }

    fn doc(id_path: &str, title: &str, date: &str) -> Doc {
        let mut d = Doc::default();
        d.id_path = PathBuf::from(id_path);
        d.title = title.to_string();
        d.date = at(date);
        d.updated = at(date);
        d
    }

    fn yaml_mapping(s: &str) -> Mapping {
        serde_yaml_ng::from_str(s).unwrap()
    }

    #[test]
    fn from_yaml_mapping_empty_is_default() {
        let q = Query::from_yaml_mapping(&Mapping::new()).unwrap();
        assert!(q.path.is_none());
        assert!(q.tag.is_none());
        assert_eq!(q.order_by, OrderKey::Created);
        assert_eq!(q.sort, SortDir::Desc);
        assert!(q.limit.is_none());
    }

    #[test]
    fn from_yaml_mapping_all_fields() {
        let m = yaml_mapping(
            "path: \"posts/*.md\"\ntag: journal\norder_by: title\nsort: asc\nlimit: 4\n",
        );
        let q = Query::from_yaml_mapping(&m).unwrap();
        assert!(q.path.is_some());
        assert_eq!(q.tag.as_deref(), Some("journal"));
        assert_eq!(q.order_by, OrderKey::Title);
        assert_eq!(q.sort, SortDir::Asc);
        assert_eq!(q.limit, Some(4));
    }

    #[test]
    fn from_yaml_mapping_unknown_key_errors() {
        let m = yaml_mapping("paht: x\n");
        assert!(Query::from_yaml_mapping(&m).is_err());
    }

    #[test]
    fn evaluate_filters_by_path_glob() {
        let docs = vec![
            doc("posts/a.md", "A", "2025-01-01"),
            doc("posts/sub/b.md", "B", "2025-01-02"),
            doc("pages/c.md", "C", "2025-01-03"),
        ];
        let q = Query::from_yaml_mapping(&yaml_mapping("path: \"posts/*.md\"\n")).unwrap();
        let results = evaluate(&q, &docs);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "A");
    }

    #[test]
    fn evaluate_filters_by_tag() {
        let mut a = doc("a.md", "A", "2025-01-01");
        a.tags = vec!["rust".into()];
        let mut b = doc("b.md", "B", "2025-01-02");
        b.tags = vec!["other".into()];
        let docs = vec![a, b];
        let q = Query::from_yaml_mapping(&yaml_mapping("tag: rust\n")).unwrap();
        let results = evaluate(&q, &docs);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "A");
    }

    #[test]
    fn evaluate_default_order_is_created_desc() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01"),
            doc("b.md", "B", "2025-02-01"),
            doc("c.md", "C", "2025-03-01"),
        ];
        let q = Query::default();
        let results = evaluate(&q, &docs);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["C", "B", "A"]);
    }

    #[test]
    fn evaluate_order_by_title_asc() {
        let docs = vec![
            doc("a.md", "Charlie", "2025-01-01"),
            doc("b.md", "Alpha", "2025-01-02"),
            doc("c.md", "Bravo", "2025-01-03"),
        ];
        let q = Query::from_yaml_mapping(&yaml_mapping("order_by: title\nsort: asc\n")).unwrap();
        let results = evaluate(&q, &docs);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn evaluate_applies_limit() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01"),
            doc("b.md", "B", "2025-02-01"),
            doc("c.md", "C", "2025-03-01"),
        ];
        let q = Query::from_yaml_mapping(&yaml_mapping("limit: 2\n")).unwrap();
        let results = evaluate(&q, &docs);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["C", "B"]);
    }
}
