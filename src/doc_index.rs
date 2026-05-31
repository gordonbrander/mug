//! The in-memory doc store threaded through every build phase. Owns all docs in
//! a `BTreeMap<PathBuf, Doc>` keyed by `id_path` — a `BTreeMap` so iteration is
//! naturally sorted by `id_path`, giving every consumer a stable order for free.
//! It is the one-stop-shop for doc iteration: phases mutate docs in parallel
//! through [`DocIndex::par_docs_mut`], read them through [`DocIndex::docs`], and
//! project the read-only [`DocMeta`] view via [`DocIndex::to_doc_metas`].
//!
//! On top of the store it caches two kinds of precomputed listings, both known
//! up front so they evaluate once and are cheap to read from templates:
//!
//! - **Collections** — a named [`Query`] result, stored as an ordered
//!   `Vec<PathBuf>` of `id_path`s. Defined from the `collections:` block in
//!   `config.yaml` (see [`DocIndex::define_collection`]).
//! - **Groups** — a keyed family of collections (`BTreeMap<key, Vec<PathBuf>>`).
//!   The only group today is `tags` (one collection per tag slug); see
//!   [`DocIndex::define_tags_group`].

use crate::doc::{Doc, DocMeta};
use crate::query::{self, Query};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Clone, Default)]
pub struct DocIndex {
    /// Keyed by `id_path`. A `BTreeMap` so `docs()`/`par_docs()` iterate in
    /// sorted `id_path` order, which keeps cached listings and outputs stable.
    docs: BTreeMap<PathBuf, Doc>,
    collections: HashMap<String, Vec<PathBuf>>,
    groups: HashMap<String, BTreeMap<String, Vec<PathBuf>>>,
}

impl DocIndex {
    pub fn new() -> DocIndex {
        DocIndex::default()
    }

    /// Insert (or replace) a doc, keyed by its `id_path`.
    pub fn insert(&mut self, doc: Doc) {
        self.docs.insert(doc.id_path.clone(), doc);
    }

    // --- iteration ---------------------------------------------------------

    /// Sequential view of every doc. Used by `write`, generator queries, and
    /// the `backlinks` adapter.
    pub fn docs(&self) -> impl Iterator<Item = &Doc> {
        self.docs.values()
    }

    /// Parallel mutable view, for the markup and template render loops.
    pub fn par_docs_mut(&mut self) -> impl ParallelIterator<Item = &mut Doc> {
        self.docs.par_iter_mut().map(|(_, doc)| doc)
    }

    /// Read-only parallel companion to [`par_docs_mut`](Self::par_docs_mut).
    pub fn par_docs(&self) -> impl ParallelIterator<Item = &Doc> {
        self.docs.par_iter().map(|(_, doc)| doc)
    }

    /// Project the read-only [`DocMeta`] view used as the frozen snapshot for the
    /// markup/generate phases. Order-independent: wikilink resolution keys off a
    /// stem index with a lexicographic tiebreak, and URL lookups are by id.
    pub fn to_doc_metas(&self) -> Vec<DocMeta> {
        self.docs.values().map(DocMeta::from).collect()
    }

    /// O(1) lookup of a doc by `id_path`.
    pub fn doc(&self, id_path: &Path) -> Option<&Doc> {
        self.docs.get(id_path)
    }

    /// O(1) lookup of a doc's `output_path` by `id_path`, for the URL filters.
    pub fn output_path(&self, id_path: &Path) -> Option<&Path> {
        self.docs.get(id_path).map(|d| d.output_path.as_path())
    }

    // --- collections -------------------------------------------------------

    /// Evaluate `query` once over every doc and cache the matching `id_path`s,
    /// in query order, under `name`.
    pub fn define_collection(&mut self, name: &str, query: &Query) {
        let ids = query::evaluate(query, self.docs.values())
            .into_iter()
            .map(|d| d.id_path.clone())
            .collect();
        self.collections.insert(name.to_string(), ids);
    }

    /// Docs of the named collection, in cached order. An unknown name yields an
    /// empty iterator.
    pub fn get_collection(&self, name: &str) -> impl Iterator<Item = &Doc> {
        self.collections
            .get(name)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.docs.get(id))
    }

    // --- groups ------------------------------------------------------------

    /// Build the `tags` group: one collection per tag slug, keyed by slug.
    pub fn define_tags_group(&mut self) {
        self.define_group("tags", |doc| doc.tags.keys().cloned().collect());
    }

    /// Generic group builder. `key_fn` returns the keys a doc belongs under; the
    /// doc's `id_path` is pushed onto each key's collection. Each collection is
    /// then ordered deterministically (date desc, `id_path` tiebreak) to match
    /// the default query order rather than raw `id_path` order.
    fn define_group<F>(&mut self, name: &str, key_fn: F)
    where
        F: Fn(&Doc) -> Vec<String>,
    {
        let mut group: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for doc in self.docs.values() {
            for key in key_fn(doc) {
                group.entry(key).or_default().push(doc.id_path.clone());
            }
        }
        for ids in group.values_mut() {
            ids.sort_by(|a, b| {
                let da = &self.docs[a];
                let db = &self.docs[b];
                db.date.cmp(&da.date).then_with(|| a.cmp(b))
            });
        }
        self.groups.insert(name.to_string(), group);
    }

    /// The named group's `key -> id_path`s map, or `None` if undefined.
    pub fn get_group(&self, name: &str) -> Option<&BTreeMap<String, Vec<PathBuf>>> {
        self.groups.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::OrderKey;
    use chrono::{DateTime, NaiveDate, Utc};

    fn at(date: &str) -> DateTime<Utc> {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
    }

    fn doc(id_path: &str, title: &str, date: &str) -> Doc {
        let mut d = Doc::default();
        d.id_path = PathBuf::from(id_path);
        d.output_path = PathBuf::from(id_path).with_extension("html");
        d.title = title.to_string();
        d.date = at(date);
        d.updated = at(date);
        d
    }

    fn index(docs: Vec<Doc>) -> DocIndex {
        let mut index = DocIndex::new();
        for d in docs {
            index.insert(d);
        }
        index
    }

    #[test]
    fn define_collection_filters_and_orders() {
        let mut index = index(vec![
            doc("posts/a.md", "A", "2025-01-01"),
            doc("posts/b.md", "B", "2025-03-01"),
            doc("pages/c.md", "C", "2025-02-01"),
        ]);
        let q = Query {
            path: Some(
                globset::GlobBuilder::new("posts/*.md")
                    .literal_separator(true)
                    .build()
                    .unwrap(),
            ),
            ..Query::default()
        };
        index.define_collection("posts", &q);
        // Default order is date desc: B (Mar) before A (Jan); C is filtered out.
        let titles: Vec<&str> = index.get_collection("posts").map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["B", "A"]);
    }

    #[test]
    fn get_collection_unknown_name_is_empty() {
        let index = index(vec![doc("a.md", "A", "2025-01-01")]);
        assert_eq!(index.get_collection("nope").count(), 0);
    }

    #[test]
    fn collection_order_is_stable_across_equal_sort_keys() {
        // Same date — the id_path tiebreak must give a deterministic order.
        let mut index = index(vec![
            doc("a.md", "A", "2025-01-01"),
            doc("b.md", "B", "2025-01-01"),
            doc("c.md", "C", "2025-01-01"),
        ]);
        let q = Query {
            order_by: OrderKey::Date,
            ..Query::default()
        };
        index.define_collection("all", &q);
        let ids: Vec<&str> = index
            .get_collection("all")
            .map(|d| d.id_path.to_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["a.md", "b.md", "c.md"]);
    }

    #[test]
    fn define_tags_group_buckets_by_slug() {
        let mut a = doc("a.md", "A", "2025-01-01");
        a.tags.insert("rust".into(), "rust".into());
        let mut b = doc("b.md", "B", "2025-03-01");
        b.tags.insert("rust".into(), "rust".into());
        b.tags.insert("go".into(), "go".into());
        let index = {
            let mut idx = index(vec![a, b]);
            idx.define_tags_group();
            idx
        };
        let group = index.get_group("tags").unwrap();
        // `rust` has both, date desc → b (Mar) before a (Jan).
        assert_eq!(
            group.get("rust").unwrap(),
            &vec![PathBuf::from("b.md"), PathBuf::from("a.md")]
        );
        assert_eq!(group.get("go").unwrap(), &vec![PathBuf::from("b.md")]);
    }

    #[test]
    fn get_group_unknown_name_is_none() {
        let index = index(vec![doc("a.md", "A", "2025-01-01")]);
        assert!(index.get_group("tags").is_none());
    }

    #[test]
    fn output_path_round_trips() {
        let index = index(vec![doc("posts/a.md", "A", "2025-01-01")]);
        assert_eq!(
            index.output_path(Path::new("posts/a.md")),
            Some(Path::new("posts/a.html"))
        );
        assert_eq!(index.output_path(Path::new("missing.md")), None);
    }

    #[test]
    fn to_doc_metas_covers_all_docs() {
        let index = index(vec![
            doc("a.md", "A", "2025-01-01"),
            doc("b.md", "B", "2025-01-02"),
        ]);
        assert_eq!(index.to_doc_metas().len(), 2);
    }
}
