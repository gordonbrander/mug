//! `dir(path="foo/bar/baz.png")` — the parent directory of a `/`-separated
//! path (`foo/bar`). Registered on both envs: it is pure string shaping, like
//! the `text` filters, with no dependency on the doc index.
//!
//! Pairs with the `filter_in_dir` filter so a template can list a page's
//! siblings — the docs sharing its directory:
//!
//! ```html
//! {% set siblings = collection(name="all")
//!      | filter_in_dir(dir=dir(path=page.id_path), omit=[page.id_path]) %}
//! ```

use std::collections::HashMap;
use tera::{Tera, Value};

pub fn register(env: &mut Tera) {
    env.register_function(
        "dir",
        |args: &HashMap<String, Value>| -> tera::Result<Value> {
            let path = match args.get("path") {
                Some(Value::String(s)) => s,
                Some(_) => return Err(tera::Error::msg("dir: `path` must be a string")),
                None => return Err(tera::Error::msg("dir: missing required `path` argument")),
            };
            Ok(Value::String(parent_dir(path).to_string()))
        },
    );
}

/// Parent directory of a `/`-separated path. `"foo/bar/baz.png"` -> `"foo/bar"`;
/// `"baz.png"` -> `""`. Trailing slashes are ignored.
///
/// Splits on `/` rather than using `Path::parent()` so behavior stays in the
/// URL/`id_path` layout and is identical across platforms (the same reasoning
/// `dirtree` documents for splitting output paths).
pub(super) fn parent_dir(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(i) => &trimmed[..i],
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_dir_nested() {
        assert_eq!(parent_dir("foo/bar/baz.png"), "foo/bar");
    }

    #[test]
    fn parent_dir_single_segment_is_empty() {
        assert_eq!(parent_dir("baz.png"), "");
    }

    #[test]
    fn parent_dir_ignores_trailing_slash() {
        assert_eq!(parent_dir("foo/bar/"), "foo");
        assert_eq!(parent_dir("foo/"), "");
    }

    #[test]
    fn dir_function_renders_parent() {
        let mut env = Tera::default();
        register(&mut env);
        let out = env
            .render_str("{{ dir(path=\"foo/bar/baz.png\") }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "foo/bar");
    }

    #[test]
    fn dir_function_top_level_is_empty() {
        let mut env = Tera::default();
        register(&mut env);
        let out = env
            .render_str("{{ dir(path=\"index.md\") }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn dir_missing_path_argument_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        assert!(
            env.render_str("{{ dir() }}", &tera::Context::new())
                .is_err()
        );
    }

    #[test]
    fn dir_non_string_path_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        assert!(
            env.render_str("{{ dir(path=5) }}", &tera::Context::new())
                .is_err()
        );
    }
}
