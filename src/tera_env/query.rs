//! Tera adapter for [`crate::query`]. Parses kwargs, forwards into
//! `query::evaluate`, serializes the result.

use crate::doc::Doc;
use crate::query::{self, KNOWN_KEYS, OrderKey, Query, SortDir};
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Tera, Value};

pub fn register(env: &mut Tera, docs: Arc<Vec<Doc>>) {
    env.register_function(
        "query",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            let q = from_kwargs(args)?;
            let results = query::evaluate(&q, docs.as_slice());
            tera::to_value(results).map_err(tera::Error::from)
        },
    );
}

/// Build a `Query` from Tera kwargs. All keys optional; unknown keys error so
/// typos surface at the call site.
fn from_kwargs(args: &HashMap<String, Value>) -> tera::Result<Query> {
    for key in args.keys() {
        if !KNOWN_KEYS.contains(&key.as_str()) {
            return Err(tera::Error::msg(format!(
                "query: unknown argument `{}` (allowed: {})",
                key,
                KNOWN_KEYS.join(", ")
            )));
        }
    }

    let mut q = Query::default();

    if let Some(v) = args.get("path") {
        let s = v
            .as_str()
            .ok_or_else(|| tera::Error::msg("query: `path` must be a string"))?;
        let glob = globset::GlobBuilder::new(s)
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
            "date" => OrderKey::Date,
            "updated" => OrderKey::Updated,
            other => {
                return Err(tera::Error::msg(format!(
                    "query: `order_by` must be one of title|date|updated (got `{}`)",
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

    if let Some(v) = args.get("omit") {
        let arr = v
            .as_array()
            .ok_or_else(|| tera::Error::msg("query: `omit` must be an array of strings"))?;
        q.omit = arr
            .iter()
            .map(|e| {
                e.as_str()
                    .map(std::path::PathBuf::from)
                    .ok_or_else(|| tera::Error::msg("query: `omit` entries must be strings"))
            })
            .collect::<tera::Result<Vec<_>>>()?;
    }

    if let Some(v) = args.get("limit") {
        let n = v
            .as_u64()
            .ok_or_else(|| tera::Error::msg("query: `limit` must be a non-negative integer"))?;
        q.limit = Some(n as usize);
    }

    Ok(q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val_str(s: &str) -> Value {
        Value::String(s.to_string())
    }

    #[test]
    fn from_kwargs_empty_is_default() {
        let q = from_kwargs(&HashMap::new()).unwrap();
        assert!(q.path.is_none());
        assert!(q.tag.is_none());
        assert_eq!(q.order_by, OrderKey::Date);
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
        let q = from_kwargs(&args).unwrap();
        assert!(q.path.is_some());
        assert_eq!(q.tag.as_deref(), Some("journal"));
        assert_eq!(q.order_by, OrderKey::Title);
        assert_eq!(q.sort, SortDir::Asc);
        assert_eq!(q.limit, Some(3));
    }

    #[test]
    fn from_kwargs_parses_omit() {
        let mut args = HashMap::new();
        args.insert(
            "omit".to_string(),
            Value::Array(vec![val_str("posts/a.md"), val_str("posts/b.md")]),
        );
        let q = from_kwargs(&args).unwrap();
        assert_eq!(
            q.omit,
            vec![
                std::path::PathBuf::from("posts/a.md"),
                std::path::PathBuf::from("posts/b.md")
            ]
        );
    }

    #[test]
    fn from_kwargs_omit_not_array_errors() {
        let mut args = HashMap::new();
        args.insert("omit".to_string(), val_str("posts/a.md"));
        assert!(from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_unknown_key_errors() {
        let mut args = HashMap::new();
        args.insert("paht".to_string(), val_str("x"));
        assert!(from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_bad_order_by_errors() {
        let mut args = HashMap::new();
        args.insert("order_by".to_string(), val_str("nope"));
        assert!(from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_bad_sort_errors() {
        let mut args = HashMap::new();
        args.insert("sort".to_string(), val_str("upward"));
        assert!(from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_bad_glob_errors() {
        let mut args = HashMap::new();
        args.insert("path".to_string(), val_str("[unterminated"));
        assert!(from_kwargs(&args).is_err());
    }
}
