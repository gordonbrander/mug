use crate::config::Config;
use crate::doc::Doc;
use crate::index::Index;
use anyhow::{Context, Result};
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
        if !matches!(ext, "md" | "html" | "yaml") {
            continue;
        }
        let id_path = path
            .strip_prefix(&config.content_dir)
            .with_context(|| format!("could not strip prefix from {}", path.display()))?
            .to_path_buf();
        let doc = Doc::load(&config.content_dir, &id_path, &config.defaults)?;
        index.insert(doc);
    }
    Ok(index)
}
