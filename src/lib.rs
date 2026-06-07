pub mod backlinks;
pub mod build;
pub mod command;
pub mod config;
pub mod copy;
pub mod doc;
pub mod doc_index;
pub mod frontmatter;
pub mod html;
pub mod permalink;
pub mod query;
pub mod related;
pub mod site_data;
pub mod taxonomy;
pub mod tera_env;
#[cfg(test)]
mod test_util;

use anyhow::Result;
use std::net::IpAddr;
use std::path::Path;

pub fn new(path: &Path) -> Result<()> {
    command::new::run(path)
}

pub fn scaffold() -> Result<()> {
    command::scaffold::run()
}

pub fn watch() -> Result<()> {
    command::watch::run()
}

pub fn serve(host: IpAddr, port: u16) -> Result<()> {
    command::serve::run(host, port)
}

pub fn clean() -> Result<()> {
    command::clean::run()
}

pub fn build(include_drafts: bool) -> Result<()> {
    build::run(include_drafts)
}
