use crate::config::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Write each `(output_path, content)` output (from [`template::run`](super::template::run))
/// into `output_dir`, creating parent directories as needed.
pub fn run(config: &Config, outputs: &[(PathBuf, String)]) -> Result<()> {
    for (output_path, content) in outputs {
        let out_path = config.output_dir.join(output_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        fs::write(&out_path, content)
            .with_context(|| format!("could not write {}", out_path.display()))?;
    }
    Ok(())
}
