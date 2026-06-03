//! Progress reporting + control surface shared by the CLI and GUI front-ends.
//!
//! The library's long-running operations ([`build`](crate::build),
//! [`serve`](crate::serve), [`watch`](crate::watch), [`clean`](crate::clean))
//! report progress through a [`Reporter`] rather than printing directly, so a GUI
//! can surface the same information the CLI writes to stderr. [`ServeHandle`] lets a
//! caller stop a running server from another thread.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Notify;

/// A single progress event emitted during a build/serve/watch/clean run.
#[derive(Debug, Clone)]
pub enum Progress {
    /// A human-readable status line (e.g. `watching for changes…`).
    Info(String),
    /// A (re)build has started.
    BuildStarted,
    /// A build finished successfully, writing `pages` output files.
    BuildOk { pages: usize, elapsed: Duration },
    /// A build failed; the string is the formatted error chain (`{:#}`).
    BuildErr(String),
    /// The dev server is bound and accepting connections on `addr`.
    ServeReady(SocketAddr),
    /// The dev server has fully stopped (clean shutdown or error exit).
    ServeStopped,
}

/// Sink for [`Progress`] events. Implementors are `Send + Sync` because
/// `serve`/`watch` report from worker threads and the Tokio runtime.
pub trait Reporter: Send + Sync {
    fn report(&self, progress: Progress);
}

/// [`Reporter`] that prints to stderr, reproducing the CLI's historical output.
pub struct StderrReporter;

impl Reporter for StderrReporter {
    fn report(&self, progress: Progress) {
        match progress {
            Progress::Info(msg) => eprintln!("{msg}"),
            Progress::BuildStarted => {}
            Progress::BuildOk { elapsed, .. } => eprintln!("rebuilt in {elapsed:?}"),
            Progress::BuildErr(msg) => eprintln!("build failed: {msg}"),
            Progress::ServeReady(addr) => eprintln!("serving http://{addr}"),
            Progress::ServeStopped => {}
        }
    }
}

/// [`Reporter`] that discards everything; used by tests and quiet callers.
pub struct NullReporter;

impl Reporter for NullReporter {
    fn report(&self, _progress: Progress) {}
}

/// Summary of a successful build, returned by [`build`](crate::build).
#[derive(Debug, Clone, Copy)]
pub struct BuildReport {
    /// Number of output files written.
    pub pages: usize,
}

/// Stop signal for a running [`serve`](crate::serve). Cloneable: hand one clone to
/// `serve` and keep another to call [`stop`](Self::stop) from elsewhere (e.g. a
/// GUI "Stop" button). Stopping is idempotent.
#[derive(Clone)]
pub struct ServeHandle {
    running: Arc<AtomicBool>,
    shutdown: Arc<Notify>,
}

impl ServeHandle {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(true)),
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Ask the server to stop: flips the running flag (polled by the watcher
    /// thread) and wakes the async accept loop. Safe to call from any thread, and
    /// safe to call before the server has started — the wake-up latches.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        // `notify_one` stores a permit if no task is waiting yet, so a `stop`
        // that races ahead of the accept loop is not lost.
        self.shutdown.notify_one();
    }

    /// The shared running flag; the watcher loop polls it to exit promptly.
    pub(crate) fn running(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    /// The shared shutdown notifier; the accept loop awaits it in `select!`.
    pub(crate) fn shutdown_notify(&self) -> Arc<Notify> {
        self.shutdown.clone()
    }
}

impl Default for ServeHandle {
    fn default() -> Self {
        Self::new()
    }
}
