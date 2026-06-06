//! Tera adapter for backlink listing. Parses kwargs into [`Backlinks`] options,
//! forwards into [`DocIndex::list_backlinks`], serializes the result.

use crate::backlinks::Backlinks;
use crate::doc_index::DocIndex;
use crate::query::{OrderKey, SortDir};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tera::{Tera, Value};

const KNOWN_KEYS: &[&str] = &["order_by", "sort", "omit", "limit"];

/// Register `backlinks` as a Tera filter on `env`. Usage:
/// `{{ doc.id_path | backlinks(order_by="title", sort="asc") }}`. The piped
/// value is the target doc's `id_path`; returns a list of docs whose
/// `links` contain it. Template-env only — spec §11 forbids index-listing
/// filters in the markup env. Reads through the shared [`DocIndex`].
pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_filter(
        "backlinks",
        move |value: &Value, args: &HashMap<String, Value>| -> tera::Result<Value> {
            let id_path_str = value.as_str().ok_or_else(|| {
                tera::Error::msg("backlinks filter: input must be a string id_path")
            })?;
            let target = Path::new(id_path_str);
            let b = from_kwargs(args)?;
            let results = index.list_backlinks(target, &b);
            tera::to_value(results).map_err(tera::Error::from)
        },
    );
}

fn from_kwargs(args: &HashMap<String, Value>) -> tera::Result<Backlinks> {
    for key in args.keys() {
        if !KNOWN_KEYS.contains(&key.as_str()) {
            return Err(tera::Error::msg(format!(
                "backlinks: unknown argument `{}` (allowed: {})",
                key,
                KNOWN_KEYS.join(", ")
            )));
        }
    }

    let mut b = Backlinks::default();

    if let Some(v) = args.get("order_by") {
        let s = v
            .as_str()
            .ok_or_else(|| tera::Error::msg("backlinks: `order_by` must be a string"))?;
        b.order_by = match s {
            "title" => OrderKey::Title,
            "date" => OrderKey::Date,
            "updated" => OrderKey::Updated,
            other => {
                return Err(tera::Error::msg(format!(
                    "backlinks: `order_by` must be one of title|date|updated (got `{}`)",
                    other
                )));
            }
        };
    }

    if let Some(v) = args.get("sort") {
        let s = v
            .as_str()
            .ok_or_else(|| tera::Error::msg("backlinks: `sort` must be a string"))?;
        b.sort = match s {
            "asc" => SortDir::Asc,
            "desc" => SortDir::Desc,
            other => {
                return Err(tera::Error::msg(format!(
                    "backlinks: `sort` must be one of asc|desc (got `{}`)",
                    other
                )));
            }
        };
    }

    if let Some(v) = args.get("omit") {
        let arr = v
            .as_array()
            .ok_or_else(|| tera::Error::msg("backlinks: `omit` must be an array of strings"))?;
        b.omit = arr
            .iter()
            .map(|e| {
                e.as_str()
                    .map(std::path::PathBuf::from)
                    .ok_or_else(|| tera::Error::msg("backlinks: `omit` entries must be strings"))
            })
            .collect::<tera::Result<Vec<_>>>()?;
    }

    if let Some(v) = args.get("limit") {
        let n = v
            .as_u64()
            .ok_or_else(|| tera::Error::msg("backlinks: `limit` must be a non-negative integer"))?;
        b.limit = Some(n as usize);
    }

    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val_str(s: &str) -> Value {
        Value::String(s.to_string())
    }

    #[test]
    fn from_kwargs_empty_is_default() {
        let b = from_kwargs(&HashMap::new()).unwrap();
        assert_eq!(b.order_by, OrderKey::Date);
        assert_eq!(b.sort, SortDir::Desc);
    }

    #[test]
    fn from_kwargs_all_fields() {
        let mut args = HashMap::new();
        args.insert("order_by".to_string(), val_str("title"));
        args.insert("sort".to_string(), val_str("asc"));
        let b = from_kwargs(&args).unwrap();
        assert_eq!(b.order_by, OrderKey::Title);
        assert_eq!(b.sort, SortDir::Asc);
    }

    #[test]
    fn from_kwargs_unknown_key_errors() {
        let mut args = HashMap::new();
        args.insert("oder_by".to_string(), val_str("title"));
        assert!(from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_rejects_path_and_tag() {
        // backlinks intentionally narrows the query surface — `path`/`tag` are
        // not meaningful here and should fail loudly so authors don't think they
        // did anything.
        for k in &["path", "tag"] {
            let mut args = HashMap::new();
            args.insert(k.to_string(), val_str("x"));
            assert!(from_kwargs(&args).is_err(), "key `{}` should error", k);
        }
    }

    #[test]
    fn from_kwargs_parses_limit() {
        let mut args = HashMap::new();
        args.insert("limit".to_string(), Value::from(3u64));
        let b = from_kwargs(&args).unwrap();
        assert_eq!(b.limit, Some(3));
    }

    #[test]
    fn from_kwargs_limit_not_integer_errors() {
        let mut args = HashMap::new();
        args.insert("limit".to_string(), val_str("3"));
        assert!(from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_parses_omit() {
        let mut args = HashMap::new();
        args.insert(
            "omit".to_string(),
            Value::Array(vec![val_str("a.md"), val_str("b.md")]),
        );
        let b = from_kwargs(&args).unwrap();
        assert_eq!(
            b.omit,
            vec![
                std::path::PathBuf::from("a.md"),
                std::path::PathBuf::from("b.md")
            ]
        );
    }

    #[test]
    fn from_kwargs_omit_not_array_errors() {
        let mut args = HashMap::new();
        args.insert("omit".to_string(), val_str("a.md"));
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
}
