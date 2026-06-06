//! Per-query options for backlink listing. The lookup itself lives on
//! [`DocIndex::list_backlinks`](crate::doc_index::DocIndex::list_backlinks),
//! which reads the cached inverted link graph; this module just owns the
//! options struct that shapes ordering and filtering of that result.

use crate::query::{OrderKey, SortDir};
use std::path::PathBuf;

pub struct Backlinks {
    pub order_by: OrderKey,
    pub sort: SortDir,
    pub omit: Vec<PathBuf>,
    pub limit: Option<usize>,
}

impl Default for Backlinks {
    fn default() -> Self {
        Self {
            order_by: OrderKey::Date,
            sort: SortDir::Desc,
            omit: Vec::new(),
            limit: None,
        }
    }
}
