//! Tera adapter for named collections. `collection(name="posts")` returns the
//! docs of the collection precomputed on the [`DocIndex`] during the template
//! phase. An unknown name returns an empty list (no error), so a misspelled name
//! renders nothing rather than failing the build.
//!
//! Optional `omit` and `limit` kwargs filter the cached collection at render
//! time — the cached docs are omit-filtered then truncated, no re-query, no
//! re-sort. `omit` layers on top of the collection's own definition-time `omit`;
//! `limit` is a render-only cap (a collection has no definition-time count — that
//! is deliberately the filter's job). This is what lets a page exclude itself
//! from a collection it belongs to, e.g.
//! `collection(name="posts", omit=[page.id_path], limit=5)`. Omit is applied
//! before limit.

use crate::doc_index::DocIndex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tera::{Tera, Value};

const KNOWN_KEYS: &[&str] = &["name", "omit", "limit"];

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "collection",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            check_known_keys(args)?;
            let name = collection_name(args)?;
            let omit = parse_omit(args)?;
            let limit = parse_limit(args)?;
            let omit: HashSet<&Path> = omit.iter().map(PathBuf::as_path).collect();
            let docs: Vec<&crate::doc::Doc> = index
                .get_collection(&name)
                .filter(|d| !omit.contains(d.id_path.as_path()))
                .take(limit.unwrap_or(usize::MAX))
                .collect();
            tera::to_value(docs).map_err(tera::Error::from)
        },
    );
}

/// Reject unknown kwargs up front so a typo fails loudly rather than silently
/// doing nothing (matches the `backlinks` adapter's guard).
fn check_known_keys(args: &HashMap<String, Value>) -> tera::Result<()> {
    for key in args.keys() {
        if !KNOWN_KEYS.contains(&key.as_str()) {
            return Err(tera::Error::msg(format!(
                "collection: unknown argument `{}` (allowed: {})",
                key,
                KNOWN_KEYS.join(", ")
            )));
        }
    }
    Ok(())
}

/// Require a string `name` kwarg. A missing or non-string `name` is an author
/// error (unlike an unknown-but-well-formed name, which simply yields nothing).
fn collection_name(args: &HashMap<String, Value>) -> tera::Result<String> {
    match args.get("name") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(tera::Error::msg("collection: `name` must be a string")),
        None => Err(tera::Error::msg(
            "collection: missing required `name` argument",
        )),
    }
}

/// Optional runtime `omit`: an array of `id_path` strings to drop from the
/// cached collection (e.g. the current page excluding itself).
fn parse_omit(args: &HashMap<String, Value>) -> tera::Result<Vec<PathBuf>> {
    match args.get("omit") {
        None => Ok(Vec::new()),
        Some(v) => {
            let arr = v.as_array().ok_or_else(|| {
                tera::Error::msg("collection: `omit` must be an array of strings")
            })?;
            arr.iter()
                .map(|e| {
                    e.as_str().map(PathBuf::from).ok_or_else(|| {
                        tera::Error::msg("collection: `omit` entries must be strings")
                    })
                })
                .collect()
        }
    }
}

/// Optional runtime `limit`: a cap applied after `omit`, on top of any
/// definition-time limit already baked into the cached order.
fn parse_limit(args: &HashMap<String, Value>) -> tera::Result<Option<usize>> {
    match args.get("limit") {
        None => Ok(None),
        Some(v) => {
            let n = v.as_u64().ok_or_else(|| {
                tera::Error::msg("collection: `limit` must be a non-negative integer")
            })?;
            Ok(Some(n as usize))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val_str(s: &str) -> Value {
        Value::String(s.to_string())
    }

    #[test]
    fn collection_name_requires_string() {
        assert!(collection_name(&HashMap::new()).is_err());
        let mut args = HashMap::new();
        args.insert("name".to_string(), Value::from(1u64));
        assert!(collection_name(&args).is_err());
        let mut args = HashMap::new();
        args.insert("name".to_string(), val_str("posts"));
        assert_eq!(collection_name(&args).unwrap(), "posts");
    }

    #[test]
    fn check_known_keys_rejects_typo() {
        let mut args = HashMap::new();
        args.insert("limti".to_string(), Value::from(2u64));
        assert!(check_known_keys(&args).is_err());
    }

    #[test]
    fn parse_omit_default_is_empty() {
        assert!(parse_omit(&HashMap::new()).unwrap().is_empty());
    }

    #[test]
    fn parse_omit_parses_array() {
        let mut args = HashMap::new();
        args.insert(
            "omit".to_string(),
            Value::Array(vec![val_str("a.md"), val_str("b.md")]),
        );
        assert_eq!(
            parse_omit(&args).unwrap(),
            vec![PathBuf::from("a.md"), PathBuf::from("b.md")]
        );
    }

    #[test]
    fn parse_omit_not_array_errors() {
        let mut args = HashMap::new();
        args.insert("omit".to_string(), val_str("a.md"));
        assert!(parse_omit(&args).is_err());
    }

    #[test]
    fn parse_limit_default_is_none() {
        assert_eq!(parse_limit(&HashMap::new()).unwrap(), None);
    }

    #[test]
    fn parse_limit_parses_integer() {
        let mut args = HashMap::new();
        args.insert("limit".to_string(), Value::from(3u64));
        assert_eq!(parse_limit(&args).unwrap(), Some(3));
    }

    #[test]
    fn parse_limit_not_integer_errors() {
        let mut args = HashMap::new();
        args.insert("limit".to_string(), val_str("3"));
        assert!(parse_limit(&args).is_err());
    }
}
