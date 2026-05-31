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
//!
//! All four are registered as *safe* filters (`is_safe() == true`): they emit
//! URLs, not arbitrary text, so their output must not be HTML-escaped by Tera's
//! autoescape in `.html`/`.xml` templates (otherwise `/` becomes `&#x2F;`).

use crate::doc::DocMeta;
use crate::doc_index::DocIndex;
use crate::permalink;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tera::{Filter, Tera, Value};

/// Resolves a doc `id_path` to its URL (`permalink::to_url(output_path)`), or
/// `None` if no such doc exists. Lets `permalink`/`link` share one filter body
/// across the markup env (which only has a `DocMeta` snapshot) and the template
/// env (which reads through the shared `DocIndex`).
pub type DocUrlLookup = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Lookup backed by the markup phase's frozen `DocMeta` snapshot.
pub fn lookup_from_metas(docs: Arc<Vec<DocMeta>>) -> DocUrlLookup {
    Arc::new(move |id_path: &str| {
        let target = PathBuf::from(id_path);
        docs.iter()
            .find(|d| d.id_path == target)
            .map(|d| permalink::to_url(&d.output_path))
    })
}

/// Lookup backed by the shared template-phase `DocIndex` (O(1), no clone).
pub fn lookup_from_index(index: Arc<DocIndex>) -> DocUrlLookup {
    Arc::new(move |id_path: &str| index.output_path(Path::new(id_path)).map(permalink::to_url))
}

/// `permalink`: doc `id_path` → absolute URL (`site_url + base_path + doc_url`).
struct PermalinkFilter {
    lookup: DocUrlLookup,
    site_url: Option<String>,
    base_path: String,
}

impl Filter for PermalinkFilter {
    fn filter(&self, value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let url = lookup_doc_url(&self.lookup, value, "permalink")?;
        let out = format!("{}{}{}", self.site_url.as_deref().unwrap_or(""), self.base_path, url);
        Ok(Value::String(out))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// `link`: doc `id_path` → root-relative URL (`base_path + doc_url`).
struct LinkFilter {
    lookup: DocUrlLookup,
    base_path: String,
}

impl Filter for LinkFilter {
    fn filter(&self, value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let url = lookup_doc_url(&self.lookup, value, "link")?;
        let out = format!("{}{}", self.base_path, url);
        Ok(Value::String(out))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// `relative_url`: arbitrary path → `base_path + "/" + path`.
struct RelativeUrlFilter {
    base_path: String,
}

impl Filter for RelativeUrlFilter {
    fn filter(&self, value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let p = value
            .as_str()
            .ok_or_else(|| tera::Error::msg("relative_url filter: input must be a string"))?;
        let stripped = p.trim_start_matches('/');
        let out = format!("{}/{}", self.base_path, stripped);
        Ok(Value::String(out))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// `absolute_url`: arbitrary path → `site_url + base_path + "/" + path`.
struct AbsoluteUrlFilter {
    site_url: Option<String>,
    base_path: String,
}

impl Filter for AbsoluteUrlFilter {
    fn filter(&self, value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let p = value
            .as_str()
            .ok_or_else(|| tera::Error::msg("absolute_url filter: input must be a string"))?;
        let stripped = p.trim_start_matches('/');
        let out = format!(
            "{}{}/{}",
            self.site_url.as_deref().unwrap_or(""),
            self.base_path,
            stripped
        );
        Ok(Value::String(out))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

pub fn register(
    env: &mut Tera,
    lookup: DocUrlLookup,
    site_url: Option<String>,
    base_path: String,
) {
    env.register_filter(
        "permalink",
        PermalinkFilter {
            lookup: lookup.clone(),
            site_url: site_url.clone(),
            base_path: base_path.clone(),
        },
    );
    env.register_filter(
        "link",
        LinkFilter {
            lookup,
            base_path: base_path.clone(),
        },
    );
    env.register_filter(
        "relative_url",
        RelativeUrlFilter {
            base_path: base_path.clone(),
        },
    );
    env.register_filter(
        "absolute_url",
        AbsoluteUrlFilter {
            site_url,
            base_path,
        },
    );
}

fn lookup_doc_url(
    lookup: &DocUrlLookup,
    value: &Value,
    filter_name: &str,
) -> tera::Result<String> {
    let id_path_str = value.as_str().ok_or_else(|| {
        tera::Error::msg(format!(
            "{} filter: input must be a string id_path",
            filter_name
        ))
    })?;
    lookup(id_path_str).ok_or_else(|| {
        tera::Error::msg(format!(
            "{} filter: no doc with id_path `{}`",
            filter_name, id_path_str
        ))
    })
}
