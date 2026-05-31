//! Tera adapter for named collections. `collection(name="posts")` returns the
//! docs of the collection precomputed on the [`DocIndex`] during the template
//! phase. An unknown name returns an empty list (no error), so a misspelled name
//! renders nothing rather than failing the build.

use crate::doc_index::DocIndex;
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Tera, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "collection",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            let name = collection_name(args)?;
            let docs: Vec<&crate::doc::Doc> = index.get_collection(&name).collect();
            tera::to_value(docs).map_err(tera::Error::from)
        },
    );
}

/// Require a string `name` kwarg. A missing or non-string `name` is an author
/// error (unlike an unknown-but-well-formed name, which simply yields nothing).
fn collection_name(args: &HashMap<String, Value>) -> tera::Result<String> {
    match args.get("name") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(tera::Error::msg("collection: `name` must be a string")),
        None => Err(tera::Error::msg("collection: missing required `name` argument")),
    }
}
