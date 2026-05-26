use crate::doc::Doc;
use anyhow::{Result, anyhow};
use globset::{Glob, GlobBuilder, GlobMatcher};
use serde_yaml_ng::{Mapping, Value as YamlValue};
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Tera, Value};

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

const KNOWN_KEYS: &[&str] = &["path", "tag", "order_by", "sort", "limit"];

impl Query {
    /// Build from Tera kwargs. All keys optional; unknown keys error so typos
    /// surface at the call site.
    pub fn from_kwargs(args: &HashMap<String, Value>) -> tera::Result<Self> {
        for key in args.keys() {
            if !KNOWN_KEYS.contains(&key.as_str()) {
                return Err(tera::Error::msg(format!(
                    "query: unknown argument `{}` (allowed: {})",
                    key,
                    KNOWN_KEYS.join(", ")
                )));
            }
        }

        let mut q = Self::default();

        if let Some(v) = args.get("path") {
            let s = v
                .as_str()
                .ok_or_else(|| tera::Error::msg("query: `path` must be a string"))?;
            let glob = GlobBuilder::new(s)
                .literal_separator(true)
                .build()
                .map_err(|e| tera::Error::msg(format!("query: invalid glob `{}`: {}", s, e)))?;
            q.path = Some(glob);
        }

        if let Some(v) = args.get("tag") {
            let s = v
                .as_str()
                .ok_or_else(|| tera::Error::msg("query: `tag` must be a string"))?;
            q.tag = Some(s.to_string());
        }

        if let Some(v) = args.get("order_by") {
            let s = v
                .as_str()
                .ok_or_else(|| tera::Error::msg("query: `order_by` must be a string"))?;
            q.order_by = match s {
                "title" => OrderKey::Title,
                "created" => OrderKey::Created,
                "updated" => OrderKey::Updated,
                other => {
                    return Err(tera::Error::msg(format!(
                        "query: `order_by` must be one of title|created|updated (got `{}`)",
                        other
                    )));
                }
            };
        }

        if let Some(v) = args.get("sort") {
            let s = v
                .as_str()
                .ok_or_else(|| tera::Error::msg("query: `sort` must be a string"))?;
            q.sort = match s {
                "asc" => SortDir::Asc,
                "desc" => SortDir::Desc,
                other => {
                    return Err(tera::Error::msg(format!(
                        "query: `sort` must be one of asc|desc (got `{}`)",
                        other
                    )));
                }
            };
        }

        if let Some(v) = args.get("limit") {
            let n = v
                .as_u64()
                .ok_or_else(|| tera::Error::msg("query: `limit` must be a non-negative integer"))?;
            q.limit = Some(n as usize);
        }

        Ok(q)
    }

    /// Build from a YAML map (e.g. the `query:` sub-mapping in a generator's
    /// frontmatter). Same field names and validation as `from_kwargs`. Kept as
    /// a sibling rather than collapsed into a single generic, because
    /// serde_yaml_ng::Value and tera::Value have different shapes and the
    /// parsing is small enough that abstraction would cost more than it saves.
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

pub fn register(env: &mut Tera, docs: Arc<Vec<Doc>>) {
    env.register_function(
        "query",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            let q = Query::from_kwargs(args)?;
            let results = evaluate(&q, docs.as_slice());
            tera::to_value(results).map_err(tera::Error::from)
        },
    );
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

    fn val_str(s: &str) -> Value {
        Value::String(s.to_string())
    }

    #[test]
    fn from_kwargs_empty_is_default() {
        let q = Query::from_kwargs(&HashMap::new()).unwrap();
        assert!(q.path.is_none());
        assert!(q.tag.is_none());
        assert_eq!(q.order_by, OrderKey::Created);
        assert_eq!(q.sort, SortDir::Desc);
        assert!(q.limit.is_none());
    }

    #[test]
    fn from_kwargs_all_fields() {
        let mut args = HashMap::new();
        args.insert("path".to_string(), val_str("posts/*.md"));
        args.insert("tag".to_string(), val_str("journal"));
        args.insert("order_by".to_string(), val_str("title"));
        args.insert("sort".to_string(), val_str("asc"));
        args.insert("limit".to_string(), tera::to_value(3u64).unwrap());
        let q = Query::from_kwargs(&args).unwrap();
        assert!(q.path.is_some());
        assert_eq!(q.tag.as_deref(), Some("journal"));
        assert_eq!(q.order_by, OrderKey::Title);
        assert_eq!(q.sort, SortDir::Asc);
        assert_eq!(q.limit, Some(3));
    }

    #[test]
    fn from_kwargs_unknown_key_errors() {
        let mut args = HashMap::new();
        args.insert("paht".to_string(), val_str("x"));
        assert!(Query::from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_bad_order_by_errors() {
        let mut args = HashMap::new();
        args.insert("order_by".to_string(), val_str("nope"));
        assert!(Query::from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_bad_sort_errors() {
        let mut args = HashMap::new();
        args.insert("sort".to_string(), val_str("upward"));
        assert!(Query::from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_bad_glob_errors() {
        let mut args = HashMap::new();
        args.insert("path".to_string(), val_str("[unterminated"));
        assert!(Query::from_kwargs(&args).is_err());
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
        let mut args = HashMap::new();
        args.insert("path".to_string(), val_str("posts/*.md"));
        let q = Query::from_kwargs(&args).unwrap();
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
        let mut args = HashMap::new();
        args.insert("tag".to_string(), val_str("rust"));
        let q = Query::from_kwargs(&args).unwrap();
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
        let mut args = HashMap::new();
        args.insert("order_by".to_string(), val_str("title"));
        args.insert("sort".to_string(), val_str("asc"));
        let q = Query::from_kwargs(&args).unwrap();
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
        let mut args = HashMap::new();
        args.insert("limit".to_string(), tera::to_value(2u64).unwrap());
        let q = Query::from_kwargs(&args).unwrap();
        let results = evaluate(&q, &docs);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["C", "B"]);
    }
}
