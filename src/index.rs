use crate::doc::Doc;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Index {
    pub docs: Vec<Doc>,
    pub by_path: BTreeMap<PathBuf, usize>,
    // pub by_tag: HashMap<String, Vec<usize>>,    // Phase 7
    // pub backlinks: HashMap<usize, Vec<usize>>,  // Phase 9
}

impl Index {
    pub fn new() -> Index {
        Index {
            docs: Vec::new(),
            by_path: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, doc: Doc) {
        let idx = self.docs.len();
        self.by_path.insert(doc.id_path.clone(), idx);
        self.docs.push(doc);
    }

    #[allow(dead_code)]
    pub fn get(&self, id_path: &Path) -> Option<&Doc> {
        self.by_path.get(id_path).map(|&i| &self.docs[i])
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}
