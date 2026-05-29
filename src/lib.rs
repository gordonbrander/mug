pub mod backlinks;
pub mod build;
pub mod command;
pub mod config;
pub mod defaults;
pub mod doc;
pub mod frontmatter;
pub mod html;
pub mod index;
pub mod permalink;
pub mod query;
pub mod site_data;
pub mod tera_env;

use anyhow::Result;
use std::net::IpAddr;
use std::path::Path;

pub fn new(path: &Path) -> Result<()> {
    command::scaffold::run(path)
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

pub fn build() -> Result<()> {
    build::run()
}
