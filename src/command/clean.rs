use crate::config::Config;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Remove the project's output directory. Returns the removed directory (so the
/// caller can report it), or `None` if there was nothing to remove.
pub fn run(root: &Path) -> Result<Option<PathBuf>> {
    let (config, _) = Config::load_with_theme(root)?;
    if config.output_dir.exists() {
        std::fs::remove_dir_all(&config.output_dir)
            .with_context(|| format!("removing {}", config.output_dir.display()))?;
        Ok(Some(config.output_dir))
    } else {
        Ok(None)
    }
}
