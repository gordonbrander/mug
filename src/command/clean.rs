use crate::config::Config;
use anyhow::{Context, Result};
use std::path::Path;

pub fn run() -> Result<()> {
    let (config, _) = Config::load_with_theme(Path::new("config.yaml"))?;
    if config.output_dir.exists() {
        std::fs::remove_dir_all(&config.output_dir)
            .with_context(|| format!("removing {}", config.output_dir.display()))?;
        eprintln!("cleaned {}", config.output_dir.display());
    }
    Ok(())
}
