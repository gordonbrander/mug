//! Watch mode: rebuild on every change to a source path or `config.yaml`.
//!
//! Per spec §2, v1 is full-rebuild only — `watch` is a debounced loop around
//! [`crate::build::run`]. Errors raised during a rebuild are reported and the
//! watcher keeps running; only watcher-setup errors propagate out of [`run`].

use crate::config::Config;
use crate::report::{BuildReport, Progress, Reporter};
use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(150);
/// How often the watch loop wakes to re-check the `running` flag while idle.
const POLL: Duration = Duration::from_millis(200);

pub fn run(root: &Path, reporter: &dyn Reporter) -> Result<()> {
    // Build once up front so the output is ready before we start watching. A
    // failed build is reported (by `rebuild`) but doesn't stop us from watching.
    let _ = rebuild(root, reporter);
    // `mug watch` runs until the process is killed, so the flag never flips.
    let running = AtomicBool::new(true);
    watch_loop(root, reporter, &running, |_| {})
}

/// Watch the source dirs of the project at `root` and rebuild on change. The
/// `on_rebuild` callback fires after each rebuild attempt with its `Result`, so
/// callers (e.g. `serve`) can react to success/failure — broadcasting a reload
/// only on `Ok`, for instance.
///
/// The loop polls `running` every [`POLL`] and exits promptly once it is cleared
/// (see [`crate::report::ServeHandle::stop`]), so a server can shut its watcher
/// thread down cleanly. This is purely the watch loop: it does *not* build once up
/// front. Callers own the initial build, because they disagree on how to handle
/// its failure — `watch` reports and keeps watching, `serve` aborts startup.
pub fn watch_loop<F>(
    root: &Path,
    reporter: &dyn Reporter,
    running: &AtomicBool,
    mut on_rebuild: F,
) -> Result<()>
where
    F: FnMut(&Result<BuildReport>),
{
    let (config, _) = Config::load_with_theme(root)?;
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(DEBOUNCE, tx)?;

    // Watch both overlay layers: the `*_roots` helpers expand to the theme's
    // subdir (when configured) and the site's, so a change to either a theme file
    // or a site override triggers a rebuild. Content and data stay site-only.
    let mut dirs: Vec<PathBuf> = vec![config.content_dir.clone(), config.data_dir.clone()];
    dirs.extend(config.template_roots());
    dirs.extend(config.archive_roots());
    dirs.extend(config.static_roots());
    for dir in &dirs {
        if dir.exists() {
            debouncer.watcher().watch(dir, RecursiveMode::Recursive)?;
        }
    }
    let config_path = root.join("config.yaml");
    if config_path.exists() {
        debouncer
            .watcher()
            .watch(&config_path, RecursiveMode::NonRecursive)?;
    }

    reporter.report(Progress::Info("watching for changes…".into()));

    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(POLL) {
            Ok(Ok(_)) => {
                let result = rebuild(root, reporter);
                on_rebuild(&result);
            }
            Ok(Err(e)) => reporter.report(Progress::Info(format!("watch error: {e}"))),
            // Idle tick: loop back and re-check `running`.
            Err(RecvTimeoutError::Timeout) => continue,
            // Debouncer dropped its sender — nothing left to watch.
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

fn rebuild(root: &Path, reporter: &dyn Reporter) -> Result<BuildReport> {
    reporter.report(Progress::BuildStarted);
    let start = Instant::now();
    // watch (and serve, which drives this loop) is the local preview, so drafts
    // are always included.
    let result = crate::build::run(root, true);
    match &result {
        Ok(report) => reporter.report(Progress::BuildOk {
            pages: report.pages,
            elapsed: start.elapsed(),
        }),
        Err(e) => reporter.report(Progress::BuildErr(format!("{e:#}"))),
    }
    result
}
