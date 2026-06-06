//! Classification: the named **collections** and **taxonomies** that templates
//! (`collection()` / `taxonomy()`) and archives read. It is split across the
//! pipeline so each half runs when its inputs are ready:
//!
//! - [`collections`] runs **before markup**. Collection membership is pure
//!   frontmatter metadata (path glob + ordering), so it needs nothing from the
//!   render. Computing it early lets the defaults phase fill each collection's
//!   members before their bodies render.
//! - [`taxonomies`] runs **after markup**, because the inline `#hashtag` pass
//!   mutates `doc.terms` during markup.
//! - [`backlinks`] runs **after markup**, because it inverts `doc.links`, which
//!   wikilink resolution populates during markup.
//!
//! Both define onto the live index in place; generated archive pages are added
//! later and are deliberately never classified, so `collection()`/`taxonomy()`
//! only ever list authored docs.

use crate::config::Config;
use crate::doc_index::DocIndex;

/// Evaluate every configured collection query and cache its membership.
pub fn collections(config: &Config, index: &mut DocIndex) {
    for (name, query) in &config.collections {
        index.define_collection(name, query);
    }
}

/// Bucket every configured taxonomy's terms.
pub fn taxonomies(config: &Config, index: &mut DocIndex) {
    index.define_taxonomies(&config.taxonomies);
}

/// Invert `doc.links` into the cached backlink graph that the `backlinks` and
/// `related` template filters read.
pub fn backlinks(index: &mut DocIndex) {
    index.define_backlinks();
}
