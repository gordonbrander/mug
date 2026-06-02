//! Shared helpers for unit tests. Compiled only under `cfg(test)`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A unique path under the system temp dir, namespaced by `label`. Uniqueness is
/// guaranteed within a process by a monotonic counter (and the pid separates
/// concurrent test binaries), so no time or RNG source is needed. The path is
/// *not* created — use [`tempdir`] when you want the directory to exist.
pub fn temp_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("mug-{label}-{}-{n}", std::process::id()))
}

/// Like [`temp_path`], but creates the directory before returning it.
pub fn tempdir(label: &str) -> PathBuf {
    let dir = temp_path(label);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Remove a temp directory and its contents, ignoring any error (e.g. already
/// gone). Used in test teardown.
pub fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}
