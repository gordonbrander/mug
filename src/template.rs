use crate::config::Config;
use crate::index::Index;
use anyhow::Result;

// Phase 3 introduces Tera rendering.
pub fn run(_config: &Config, _index: &mut Index) -> Result<()> {
    Ok(())
}
