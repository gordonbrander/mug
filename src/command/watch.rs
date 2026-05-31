//! Watch mode: rebuild on every change to a source path or `config.yaml`.
//!
//! Per spec §2, v1 is full-rebuild only — `watch` is a debounced loop around
//! [`crate::build::run`]. Errors raised during a rebuild are logged and the
//! watcher keeps running; only watcher-setup errors propagate out of [`run`].

use crate::config::Config;
use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(150);

pub fn run() -> Result<()> {
    // Build once up front so the output is ready before we start watching. A
    // failed build is logged (by `rebuild`) but doesn't stop us from watching.
    let _ = rebuild();
    watch_loop(|_| {})
}

/// Watch the source dirs and rebuild on change. The `on_rebuild` callback
/// fires after each rebuild attempt with its `Result`, so callers (e.g.
/// `serve`) can react to success/failure — broadcasting a reload only on `Ok`,
/// for instance.
///
/// This is purely the watch loop: it does *not* build once up front. Callers
/// own the initial build, because they disagree on how to handle its failure —
/// `watch` logs and keeps watching, `serve` aborts startup (`build::run()?`).
pub fn watch_loop<F>(mut on_rebuild: F) -> Result<()>
where
    F: FnMut(&Result<()>),
{
    let (config, _) = Config::load(Path::new("config.yaml"))?;
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(DEBOUNCE, tx)?;

    let dirs = [
        &config.content_dir,
        &config.templates_dir,
        &config.archives_dir,
        &config.data_dir,
        &config.static_dir,
    ];
    for dir in dirs {
        if dir.exists() {
            debouncer.watcher().watch(dir, RecursiveMode::Recursive)?;
        }
    }
    let config_path = Path::new("config.yaml");
    if config_path.exists() {
        debouncer
            .watcher()
            .watch(config_path, RecursiveMode::NonRecursive)?;
    }

    eprintln!("watching for changes…");

    for batch in rx {
        match batch {
            Ok(_) => {
                let result = rebuild();
                on_rebuild(&result);
            }
            Err(e) => eprintln!("watch error: {e}"),
        }
    }
    Ok(())
}

fn rebuild() -> Result<()> {
    let start = Instant::now();
    let result = crate::build::run();
    match &result {
        Ok(()) => eprintln!("rebuilt in {:?}", start.elapsed()),
        Err(e) => eprintln!("build failed: {e:#}"),
    }
    result
}
