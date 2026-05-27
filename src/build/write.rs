use crate::config::Config;
use crate::index::Index;
use anyhow::{Context, Result};
use std::fs;

pub fn run(config: &Config, index: &Index) -> Result<()> {
    for doc in &index.docs {
        let out_path = config.output_dir.join(&doc.output_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        fs::write(&out_path, &doc.content)
            .with_context(|| format!("could not write {}", out_path.display()))?;
    }
    Ok(())
}
