use crate::config::Config;
use anyhow::{Context, Result};
use std::fs;
use walkdir::WalkDir;

/// Recursively copy every file under `config.static_dir` into
/// `config.output_dir`, preserving subpaths. A missing static dir is a no-op
/// so zero-config sites still build.
pub fn run(config: &Config) -> Result<()> {
    if !config.static_dir.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(&config.static_dir) {
        let entry = entry.with_context(|| {
            format!("walking {}", config.static_dir.display())
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&config.static_dir)
            .expect("walkdir entry is always under the root we passed in");
        let dest = config.output_dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::copy(entry.path(), &dest).with_context(|| {
            format!("copying {} -> {}", entry.path().display(), dest.display())
        })?;
    }
    Ok(())
}
