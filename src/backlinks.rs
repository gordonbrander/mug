use crate::doc::Doc;
use crate::query::{OrderKey, SortDir};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tera::{Tera, Value};

pub struct Backlinks {
    pub order_by: OrderKey,
    pub sort: SortDir,
}

impl Default for Backlinks {
    fn default() -> Self {
        Self {
            order_by: OrderKey::Created,
            sort: SortDir::Desc,
        }
    }
}

const KNOWN_KEYS: &[&str] = &["order_by", "sort"];

impl Backlinks {
    pub fn from_kwargs(args: &HashMap<String, Value>) -> tera::Result<Self> {
        for key in args.keys() {
            if !KNOWN_KEYS.contains(&key.as_str()) {
                return Err(tera::Error::msg(format!(
                    "backlinks: unknown argument `{}` (allowed: {})",
                    key,
                    KNOWN_KEYS.join(", ")
                )));
            }
        }

        let mut b = Self::default();

        if let Some(v) = args.get("order_by") {
            let s = v
                .as_str()
                .ok_or_else(|| tera::Error::msg("backlinks: `order_by` must be a string"))?;
            b.order_by = match s {
                "title" => OrderKey::Title,
                "created" => OrderKey::Created,
                "updated" => OrderKey::Updated,
                other => {
                    return Err(tera::Error::msg(format!(
                        "backlinks: `order_by` must be one of title|created|updated (got `{}`)",
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

        Ok(b)
    }
}

/// Linear scan over `docs`: collect any whose `outlinks` contain `target`,
/// then sort. No persistent inverted graph — the spec §11 "fully populated
/// before listing" invariant is what makes this correct.
pub fn list_backlinks<'a>(docs: &'a [Doc], target: &Path, b: &Backlinks) -> Vec<&'a Doc> {
    let mut results: Vec<&Doc> = docs
        .iter()
        .filter(|d| d.outlinks.iter().any(|o| o == target))
        .collect();

    results.sort_by(|a, b2| {
        let cmp = match b.order_by {
            OrderKey::Title => a.title.cmp(&b2.title),
            OrderKey::Created => a.date.cmp(&b2.date),
            OrderKey::Updated => a.updated.cmp(&b2.updated),
        };
        match b.sort {
            SortDir::Asc => cmp,
            SortDir::Desc => cmp.reverse(),
        }
    });

    results
}

/// Register `backlinks` as a Tera filter on `env`. Usage:
/// `{{ doc.id_path | backlinks(order_by="title", sort="asc") }}`. The piped
/// value is the target doc's `id_path`; returns a list of docs whose
/// `outlinks` contain it. Template-env only — spec §11 forbids index-listing
/// filters in the markup env.
pub fn register(env: &mut Tera, docs: Arc<Vec<Doc>>) {
    env.register_filter(
        "backlinks",
        move |value: &Value, args: &HashMap<String, Value>| -> tera::Result<Value> {
            let id_path_str = value.as_str().ok_or_else(|| {
                tera::Error::msg("backlinks filter: input must be a string id_path")
            })?;
            let target = Path::new(id_path_str);
            let b = Backlinks::from_kwargs(args)?;
            let results = list_backlinks(docs.as_slice(), target, &b);
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

    fn doc(id_path: &str, title: &str, date: &str, outlinks: &[&str]) -> Doc {
        let mut d = Doc::default();
        d.id_path = PathBuf::from(id_path);
        d.title = title.to_string();
        d.date = at(date);
        d.updated = at(date);
        d.outlinks = outlinks.iter().map(PathBuf::from).collect();
        d
    }

    fn val_str(s: &str) -> Value {
        Value::String(s.to_string())
    }

    #[test]
    fn from_kwargs_empty_is_default() {
        let b = Backlinks::from_kwargs(&HashMap::new()).unwrap();
        assert_eq!(b.order_by, OrderKey::Created);
        assert_eq!(b.sort, SortDir::Desc);
    }

    #[test]
    fn from_kwargs_all_fields() {
        let mut args = HashMap::new();
        args.insert("order_by".to_string(), val_str("title"));
        args.insert("sort".to_string(), val_str("asc"));
        let b = Backlinks::from_kwargs(&args).unwrap();
        assert_eq!(b.order_by, OrderKey::Title);
        assert_eq!(b.sort, SortDir::Asc);
    }

    #[test]
    fn from_kwargs_unknown_key_errors() {
        let mut args = HashMap::new();
        args.insert("oder_by".to_string(), val_str("title"));
        assert!(Backlinks::from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_rejects_path_and_tag_and_limit() {
        // backlinks intentionally narrows the query surface to order_by/sort —
        // other kwargs should fail loudly so authors don't think they did
        // anything.
        for k in &["path", "tag", "limit"] {
            let mut args = HashMap::new();
            args.insert(k.to_string(), val_str("x"));
            assert!(Backlinks::from_kwargs(&args).is_err(), "key `{}` should error", k);
        }
    }

    #[test]
    fn from_kwargs_bad_order_by_errors() {
        let mut args = HashMap::new();
        args.insert("order_by".to_string(), val_str("nope"));
        assert!(Backlinks::from_kwargs(&args).is_err());
    }

    #[test]
    fn from_kwargs_bad_sort_errors() {
        let mut args = HashMap::new();
        args.insert("sort".to_string(), val_str("upward"));
        assert!(Backlinks::from_kwargs(&args).is_err());
    }

    #[test]
    fn list_backlinks_no_backlinks_returns_empty() {
        let docs = vec![doc("a.md", "A", "2025-01-01", &[])];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("b.md"), &b);
        assert!(results.is_empty());
    }

    #[test]
    fn list_backlinks_finds_single_backlink() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01", &["b.md"]),
            doc("b.md", "B", "2025-01-02", &[]),
        ];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("b.md"), &b);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "A");
    }

    #[test]
    fn list_backlinks_finds_multiple_backlinks() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01", &["b.md"]),
            doc("c.md", "C", "2025-01-03", &["b.md", "other.md"]),
            doc("d.md", "D", "2025-01-02", &["other.md"]),
        ];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("b.md"), &b);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn list_backlinks_default_order_is_created_desc() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01", &["target.md"]),
            doc("b.md", "B", "2025-02-01", &["target.md"]),
            doc("c.md", "C", "2025-03-01", &["target.md"]),
        ];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("target.md"), &b);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["C", "B", "A"]);
    }

    #[test]
    fn list_backlinks_order_by_title_asc() {
        let docs = vec![
            doc("a.md", "Charlie", "2025-01-01", &["target.md"]),
            doc("b.md", "Alpha", "2025-01-02", &["target.md"]),
            doc("c.md", "Bravo", "2025-01-03", &["target.md"]),
        ];
        let mut args = HashMap::new();
        args.insert("order_by".to_string(), val_str("title"));
        args.insert("sort".to_string(), val_str("asc"));
        let b = Backlinks::from_kwargs(&args).unwrap();
        let results = list_backlinks(&docs, Path::new("target.md"), &b);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn list_backlinks_does_not_include_self_unless_self_outlinks() {
        // Source linking to itself should still appear in its own backlinks.
        let docs = vec![doc("a.md", "A", "2025-01-01", &["a.md"])];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("a.md"), &b);
        assert_eq!(results.len(), 1);
    }
}
