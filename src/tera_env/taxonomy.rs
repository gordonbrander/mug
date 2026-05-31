//! Tera adapter for taxonomies. `taxonomy(name="tags")` returns the whole
//! taxonomy as an object keyed by term (slug) → list of docs, so templates can
//! iterate `{% for term, docs in taxonomy(name="tags") %}` or index a single
//! term `taxonomy(name="tags")["rust"]`. An unknown name returns an empty
//! object. Only source docs are classified, so generated archive pages never
//! appear here.

use crate::doc::Doc;
use crate::doc_index::DocIndex;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tera::{Tera, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "taxonomy",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            let name = taxonomy_name(args)?;
            // Resolve each term's cached id list back to docs. Unknown taxonomy
            // name → empty map. A `BTreeMap` keeps term iteration sorted.
            let mut out: BTreeMap<String, Vec<&Doc>> = BTreeMap::new();
            if let Some(taxonomy) = index.get_taxonomy(&name) {
                for (term, ids) in taxonomy {
                    let docs: Vec<&Doc> = ids.iter().filter_map(|id| index.doc(id)).collect();
                    out.insert(term.clone(), docs);
                }
            }
            tera::to_value(out).map_err(tera::Error::from)
        },
    );
}

fn taxonomy_name(args: &HashMap<String, Value>) -> tera::Result<String> {
    match args.get("name") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(tera::Error::msg("taxonomy: `name` must be a string")),
        None => Err(tera::Error::msg("taxonomy: missing required `name` argument")),
    }
}
