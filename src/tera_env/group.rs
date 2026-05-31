//! Tera adapter for groups. `group(name="tags")` returns the whole group as an
//! object keyed by group key (tag slug) → list of docs, so templates can iterate
//! `{% for tag, docs in group(name="tags") %}` or index a single key
//! `group(name="tags")["rust"]`. An unknown name returns an empty object.

use crate::doc::Doc;
use crate::doc_index::DocIndex;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tera::{Tera, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "group",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            let name = group_name(args)?;
            // Resolve each key's cached id list back to docs. Unknown group name
            // → empty map. A `BTreeMap` keeps key iteration sorted/deterministic.
            let mut out: BTreeMap<String, Vec<&Doc>> = BTreeMap::new();
            if let Some(group) = index.get_group(&name) {
                for (key, ids) in group {
                    let docs: Vec<&Doc> =
                        ids.iter().filter_map(|id| index.doc(id)).collect();
                    out.insert(key.clone(), docs);
                }
            }
            tera::to_value(out).map_err(tera::Error::from)
        },
    );
}

fn group_name(args: &HashMap<String, Value>) -> tera::Result<String> {
    match args.get("name") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(tera::Error::msg("group: `name` must be a string")),
        None => Err(tera::Error::msg("group: missing required `name` argument")),
    }
}
