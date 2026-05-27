//! General-purpose text-shaping filters, registered on both envs.
//!
//! - `truncate_words` — `text | truncate_words(length=N)`. Truncates at the
//!   last whitespace that fits, appending `…` when truncation happens. Default
//!   length is 250. Complements Tera's built-in `striptags`, which strips
//!   HTML, and `truncate`, which is not word-aware.

use crate::html;
use std::collections::HashMap;
use tera::{Tera, Value};

pub fn register(env: &mut Tera) {
    env.register_filter(
        "truncate_words",
        |value: &Value, args: &HashMap<String, Value>| -> tera::Result<Value> {
            let text = value
                .as_str()
                .ok_or_else(|| tera::Error::msg("truncate_words filter: input must be a string"))?;
            let length = args
                .get("length")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
                .unwrap_or(250);
            tera::to_value(html::truncate_words(text, length)).map_err(tera::Error::from)
        },
    );
}
