//! Tera adapter for single-doc lookup. `doc(id_path="posts/hello.md")` returns
//! the full doc with that `id_path` from the frozen [`DocIndex`] shared across
//! the template phase. An unknown `id_path` returns `null` (no error), so a
//! template can guard with `{% if doc(id_path=...) %}` rather than failing the
//! build — mirroring `collection`'s lenient handling of an unknown name.

use crate::doc_index::DocIndex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tera::{Tera, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "doc",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            let id_path = id_path_arg(args)?;
            match index.doc(Path::new(&id_path)) {
                Some(doc) => tera::to_value(doc).map_err(tera::Error::from),
                None => Ok(Value::Null),
            }
        },
    );
}

/// Require a string `id_path` kwarg. A missing or non-string `id_path` is an
/// author error (unlike an unknown-but-well-formed path, which yields `null`).
fn id_path_arg(args: &HashMap<String, Value>) -> tera::Result<String> {
    match args.get("id_path") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(tera::Error::msg("doc: `id_path` must be a string")),
        None => Err(tera::Error::msg("doc: missing required `id_path` argument")),
    }
}
