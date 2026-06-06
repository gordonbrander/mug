//! `docs | filter_in_dir(dir="foo/bar", omit=[...])` — keep only the docs whose
//! `id_path` is an *immediate* child of `dir`. Registered on both envs: it reads
//! `id_path` off each piped doc value and never touches the index, so it is pure
//! data shaping like `dirtree`.
//!
//! `dir` is a literal directory; it is **not** auto-derived from a file path.
//! Wrap a page's path with the `dir()` function to get its directory:
//!
//! ```html
//! {% set siblings = collection(name="all")
//!      | filter_in_dir(dir=dir(path=page.id_path), omit=[page.id_path]) %}
//! ```
//!
//! Matching is against `id_path` (the source path, the same field `dir()`
//! operates on). Results are sorted by `id_path` ascending, like the order
//! `dirtree`/`entries` produce, rather than preserving the piped order.

use super::dir::parent_dir;
use std::collections::{HashMap, HashSet};
use tera::{Tera, Value};

const KNOWN_KEYS: &[&str] = &["dir", "omit"];

pub fn register(env: &mut Tera) {
    env.register_filter(
        "filter_in_dir",
        |value: &Value, args: &HashMap<String, Value>| -> tera::Result<Value> {
            check_known_keys(args)?;
            let docs = value.as_array().ok_or_else(|| {
                tera::Error::msg("filter_in_dir filter: input must be an array of docs")
            })?;
            let dir = dir_arg(args)?;
            let omit = parse_omit(args)?;
            let omit: HashSet<&str> = omit.iter().map(String::as_str).collect();
            filter_in_dir(docs, &dir, &omit)
        },
    );
}

/// Reject unknown kwargs so a typo fails loudly rather than silently doing
/// nothing (matches the `collection`/`backlinks` adapters' guard).
fn check_known_keys(args: &HashMap<String, Value>) -> tera::Result<()> {
    for key in args.keys() {
        if !KNOWN_KEYS.contains(&key.as_str()) {
            return Err(tera::Error::msg(format!(
                "filter_in_dir: unknown argument `{}` (allowed: {})",
                key,
                KNOWN_KEYS.join(", ")
            )));
        }
    }
    Ok(())
}

/// Require a string `dir` kwarg. An empty string selects top-level docs.
fn dir_arg(args: &HashMap<String, Value>) -> tera::Result<String> {
    match args.get("dir") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(tera::Error::msg("filter_in_dir: `dir` must be a string")),
        None => Err(tera::Error::msg(
            "filter_in_dir: missing required `dir` argument",
        )),
    }
}

/// Optional `omit`: an array of `id_path` strings to drop (e.g. the current page
/// excluding itself). Mirrors `collection`'s `parse_omit`.
fn parse_omit(args: &HashMap<String, Value>) -> tera::Result<Vec<String>> {
    match args.get("omit") {
        None => Ok(Vec::new()),
        Some(v) => {
            let arr = v.as_array().ok_or_else(|| {
                tera::Error::msg("filter_in_dir: `omit` must be an array of strings")
            })?;
            arr.iter()
                .map(|e| {
                    e.as_str().map(str::to_string).ok_or_else(|| {
                        tera::Error::msg("filter_in_dir: `omit` entries must be strings")
                    })
                })
                .collect()
        }
    }
}

/// Keep docs whose `id_path` parent equals `dir` and that are not in `omit`,
/// returned sorted by `id_path` ascending. Kept free of Tera plumbing so it can
/// be unit-tested directly.
fn filter_in_dir(docs: &[Value], dir: &str, omit: &HashSet<&str>) -> tera::Result<Value> {
    let mut kept: Vec<(&str, &Value)> = Vec::new();
    for doc in docs {
        let id_path = doc.get("id_path").and_then(Value::as_str).ok_or_else(|| {
            tera::Error::msg("filter_in_dir filter: every doc must have a string `id_path`")
        })?;
        if parent_dir(id_path) == dir && !omit.contains(id_path) {
            kept.push((id_path, doc));
        }
    }
    kept.sort_by(|a, b| a.0.cmp(b.0));
    Ok(Value::Array(
        kept.into_iter().map(|(_, d)| d.clone()).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A doc value with just the fields `filter_in_dir` reads/echoes.
    fn doc(id_path: &str, title: &str) -> Value {
        let map: BTreeMap<&str, &str> = [("id_path", id_path), ("title", title)]
            .into_iter()
            .collect();
        tera::to_value(map).unwrap()
    }

    fn ids(value: &Value) -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["id_path"].as_str().unwrap().to_string())
            .collect()
    }

    fn empty_omit() -> HashSet<&'static str> {
        HashSet::new()
    }

    #[test]
    fn keeps_only_immediate_children_sorted() {
        let docs = [
            doc("foo/bar/b.md", "B"),
            doc("foo/bar/a.md", "A"),
            doc("foo/bar/sub/deep.md", "Deep"), // nested — excluded
            doc("foo/other.md", "Other"),       // different dir — excluded
        ];
        let out = filter_in_dir(&docs, "foo/bar", &empty_omit()).unwrap();
        // Sorted by id_path ascending, immediate children only.
        assert_eq!(ids(&out), vec!["foo/bar/a.md", "foo/bar/b.md"]);
    }

    #[test]
    fn empty_dir_selects_top_level() {
        let docs = [
            doc("index.md", "Home"),
            doc("about.md", "About"),
            doc("posts/a.md", "A"), // has a parent dir — excluded
        ];
        let out = filter_in_dir(&docs, "", &empty_omit()).unwrap();
        assert_eq!(ids(&out), vec!["about.md", "index.md"]);
    }

    #[test]
    fn omit_drops_listed_id_paths() {
        let docs = [doc("foo/a.md", "A"), doc("foo/b.md", "B")];
        let omit: HashSet<&str> = ["foo/a.md"].into_iter().collect();
        let out = filter_in_dir(&docs, "foo", &omit).unwrap();
        assert_eq!(ids(&out), vec!["foo/b.md"]);
    }

    #[test]
    fn missing_id_path_is_an_error() {
        let no_id = tera::to_value(BTreeMap::from([("title", "No id")])).unwrap();
        assert!(filter_in_dir(&[no_id], "", &empty_omit()).is_err());
    }

    #[test]
    fn non_array_input_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        let mut ctx = tera::Context::new();
        ctx.insert("x", "not an array");
        assert!(
            env.render_str("{{ x | filter_in_dir(dir=\"foo\") }}", &ctx)
                .is_err()
        );
    }

    #[test]
    fn missing_dir_argument_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        let mut ctx = tera::Context::new();
        ctx.insert("docs", &Vec::<Value>::new());
        assert!(env.render_str("{{ docs | filter_in_dir }}", &ctx).is_err());
    }

    #[test]
    fn unknown_argument_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        let mut ctx = tera::Context::new();
        ctx.insert("docs", &Vec::<Value>::new());
        assert!(
            env.render_str("{{ docs | filter_in_dir(dir=\"x\", limit=3) }}", &ctx)
                .is_err()
        );
    }
}
