//! Jekyll-inspired URL filters. Registered on both the markup and template
//! envs (spec §11): all four are either 1:1 doc lookups or string composition,
//! never index listings.
//!
//! - `permalink` — `id_path` → absolute URL (`site_url + base_path + doc_url`).
//!   Falls back to root-relative output when `site_url` is `None`.
//! - `link` — `id_path` → root-relative URL (`base_path + doc_url`).
//! - `relative_url` — arbitrary relative path → `base_path + "/" + path`.
//! - `absolute_url` — arbitrary relative path → `site_url + base_path + "/" + path`.
//!   Falls back to root-relative when `site_url` is `None`.

use crate::doc::DocMeta;
use crate::permalink;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tera::{Tera, Value};

pub fn register(
    env: &mut Tera,
    docs: Arc<Vec<DocMeta>>,
    site_url: Option<String>,
    base_path: String,
) {
    // permalink: id_path -> absolute (site_url + base_path + url)
    let docs_p = docs.clone();
    let site_p = site_url.clone();
    let base_p = base_path.clone();
    env.register_filter(
        "permalink",
        move |value: &Value, _args: &HashMap<String, Value>| -> tera::Result<Value> {
            let url = lookup_doc_url(&docs_p, value, "permalink")?;
            let out = format!(
                "{}{}{}",
                site_p.as_deref().unwrap_or(""),
                base_p,
                url
            );
            tera::to_value(out).map_err(tera::Error::from)
        },
    );

    // link: id_path -> root-relative (base_path + url)
    let docs_l = docs;
    let base_l = base_path.clone();
    env.register_filter(
        "link",
        move |value: &Value, _args: &HashMap<String, Value>| -> tera::Result<Value> {
            let url = lookup_doc_url(&docs_l, value, "link")?;
            let out = format!("{}{}", base_l, url);
            tera::to_value(out).map_err(tera::Error::from)
        },
    );

    // relative_url: path -> base_path + "/" + path
    let base_r = base_path.clone();
    env.register_filter(
        "relative_url",
        move |value: &Value, _args: &HashMap<String, Value>| -> tera::Result<Value> {
            let p = value.as_str().ok_or_else(|| {
                tera::Error::msg("relative_url filter: input must be a string")
            })?;
            let stripped = p.trim_start_matches('/');
            let out = format!("{}/{}", base_r, stripped);
            tera::to_value(out).map_err(tera::Error::from)
        },
    );

    // absolute_url: path -> site_url + base_path + "/" + path
    env.register_filter(
        "absolute_url",
        move |value: &Value, _args: &HashMap<String, Value>| -> tera::Result<Value> {
            let p = value.as_str().ok_or_else(|| {
                tera::Error::msg("absolute_url filter: input must be a string")
            })?;
            let stripped = p.trim_start_matches('/');
            let out = format!(
                "{}{}/{}",
                site_url.as_deref().unwrap_or(""),
                base_path,
                stripped
            );
            tera::to_value(out).map_err(tera::Error::from)
        },
    );
}

fn lookup_doc_url(
    docs: &[DocMeta],
    value: &Value,
    filter_name: &str,
) -> tera::Result<String> {
    let id_path_str = value.as_str().ok_or_else(|| {
        tera::Error::msg(format!(
            "{} filter: input must be a string id_path",
            filter_name
        ))
    })?;
    let target = PathBuf::from(id_path_str);
    let doc = docs.iter().find(|d| d.id_path == target).ok_or_else(|| {
        tera::Error::msg(format!(
            "{} filter: no doc with id_path `{}`",
            filter_name, id_path_str
        ))
    })?;
    Ok(permalink::to_url(&doc.output_path))
}
