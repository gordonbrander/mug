//! `entries` — turn a map into a key-sorted array, registered on both envs.
//!
//! Tera has no built-in for iterating a map in a defined order (its `sort`
//! filter only takes arrays). `map | entries` emits an array of
//! `{key, value}` objects sorted by `key`, so templates can walk a
//! `taxonomy(...)` map or a `group_by` result deterministically:
//!
//! ```html
//! {% for entry in tags | entries(sort="desc") %}
//!   {{ entry.key }}: {{ entry.value | length }}
//! {% endfor %}
//! ```
//!
//! `sort` is `"asc"` (default) or `"desc"`; any other value is an author error.

use std::collections::HashMap;
use tera::{Map, Tera, Value};

pub fn register(env: &mut Tera) {
    env.register_filter(
        "entries",
        |value: &Value, args: &HashMap<String, Value>| -> tera::Result<Value> {
            let map = value
                .as_object()
                .ok_or_else(|| tera::Error::msg("entries filter: input must be a map"))?;
            let descending = sort_is_descending(args)?;

            // serde_json::Map iterates in insertion order; collect and sort by
            // key so output is deterministic regardless of how the map was built.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            if descending {
                keys.reverse();
            }

            let entries: Vec<Value> = keys
                .into_iter()
                .map(|k| {
                    let mut entry = Map::with_capacity(2);
                    entry.insert("key".to_string(), Value::String(k.clone()));
                    entry.insert("value".to_string(), map[k].clone());
                    Value::Object(entry)
                })
                .collect();

            Ok(Value::Array(entries))
        },
    );
}

/// `false` for `"asc"` (the default when omitted), `true` for `"desc"`. Any
/// other value is an author error, mirroring how the URL/`doc` adapters treat
/// malformed arguments rather than silently guessing.
fn sort_is_descending(args: &HashMap<String, Value>) -> tera::Result<bool> {
    match args.get("sort") {
        None => Ok(false),
        Some(Value::String(s)) if s == "asc" => Ok(false),
        Some(Value::String(s)) if s == "desc" => Ok(true),
        Some(_) => Err(tera::Error::msg(
            "entries filter: `sort` must be \"asc\" or \"desc\"",
        )),
    }
}
