use crate::config::Config;
use crate::doc::Doc;
use crate::index::Index;
use anyhow::{Context, Result};
use std::fs;
use walkdir::WalkDir;

pub fn run(config: &Config) -> Result<Index> {
    let mut index = Index::new();
    if !config.content_dir.exists() {
        return Ok(index);
    }
    for entry in WalkDir::new(&config.content_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        // Phase 4 will add .html and .yaml.
        if ext != "md" {
            continue;
        }
        let id_path = path
            .strip_prefix(&config.content_dir)
            .with_context(|| format!("could not strip prefix from {}", path.display()))?
            .to_path_buf();
        let body = fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        index.insert(Doc::from_body(id_path, body));
    }
    Ok(index)
}
