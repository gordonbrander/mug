pub mod backlinks;
pub mod build;
pub mod command;
pub mod config;
pub mod doc;
pub mod doc_index;
pub mod frontmatter;
pub mod html;
pub mod permalink;
pub mod query;
pub mod report;
pub mod site_data;
pub mod taxonomy;
pub mod tera_env;
#[cfg(test)]
mod test_util;

use anyhow::Result;
use report::{BuildReport, Reporter, ServeHandle};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Scaffold a new starter site at `path` (which must not yet exist).
pub fn new(path: &Path) -> Result<()> {
    command::scaffold::run(path)
}

/// Watch the project at `root` and rebuild on change until interrupted, reporting
/// progress through `reporter`.
pub fn watch(root: &Path, reporter: Arc<dyn Reporter>) -> Result<()> {
    command::watch::run(root, reporter.as_ref())
}

/// Serve the project at `root` with live reload, reporting through `reporter`.
/// Call [`ServeHandle::stop`] on `handle` (from any thread) to shut it down.
pub fn serve(
    root: &Path,
    host: IpAddr,
    port: u16,
    reporter: Arc<dyn Reporter>,
    handle: ServeHandle,
) -> Result<()> {
    command::serve::run(root, host, port, reporter, handle)
}

/// Remove the output directory of the project at `root`. Returns the removed
/// directory, or `None` if there was nothing to remove.
pub fn clean(root: &Path) -> Result<Option<PathBuf>> {
    command::clean::run(root)
}

/// Build the project at `root` once, returning a [`BuildReport`].
pub fn build(root: &Path, include_drafts: bool) -> Result<BuildReport> {
    build::run(root, include_drafts)
}
