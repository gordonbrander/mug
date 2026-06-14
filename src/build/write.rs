use crate::build::Output;
use crate::config::Config;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Write each [`Output`] (from [`template::run`](super::template::run) and
/// [`alias::run`](super::alias::run)) into `output_dir`, creating parent
/// directories as needed.
///
/// Two outputs claiming the same `output_path` is a misconfiguration (clashing
/// `permalink`s, or an alias shadowing a real page). The **first** writer of a
/// path wins and any later claimant is skipped with a warning naming both docs.
/// Which real page wins such a collision is arbitrary (iteration order is
/// unspecified) — it is a bug to fix, so the warning, not the winner, is the
/// point. Because the pipeline appends alias stubs after all real pages, a
/// redirect stub can never clobber a real page regardless of order.
pub fn run(config: &Config, outputs: &[Output]) -> Result<()> {
    let mut written: HashMap<&PathBuf, &PathBuf> = HashMap::new();
    for output in outputs {
        if let Some(winner) = written.get(&output.output_path) {
            eprintln!(
                "duplicate output path '{}' from {} — already written by {}; skipping",
                output.output_path.display(),
                output.id_path.display(),
                winner.display(),
            );
            continue;
        }
        let out_path = config.output_dir.join(&output.output_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        fs::write(&out_path, &output.content)
            .with_context(|| format!("could not write {}", out_path.display()))?;
        written.insert(&output.output_path, &output.id_path);
    }
    Ok(())
}
