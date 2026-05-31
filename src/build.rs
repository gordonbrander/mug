//! The site build pipeline. Stages run in order, each consuming the index
//! the previous stage left behind:
//!
//! 1. [`read`] — scan `content/` into a [`DocIndex`](crate::doc_index::DocIndex).
//! 2. [`markup`] — render markdown bodies through Tera + comrak.
//! 3. [`generate`] — run `generators/` and append the emitted docs.
//! 4. [`template`] — apply each doc's Tera template.
//! 5. [`write`] — write rendered bodies to `output_dir`.
//! 6. [`static_copy`] — copy `static/` over the top.

pub mod generate;
pub mod generator;
pub mod markup;
pub mod read;
pub mod static_copy;
pub mod template;
pub mod write;

use crate::config::Config;
use crate::site_data::SiteData;
use anyhow::Result;
use std::path::Path;

pub fn run() -> Result<()> {
    let (config, site) = Config::load(Path::new("config.yaml"))?;
    let site_data = SiteData::load(&config, site)?;
    let mut index = read::run(&config)?;
    markup::run(&config, &site_data, &mut index)?;
    generate::run(&config, &site_data, &mut index)?;
    template::run(&config, &site_data, &mut index)?;
    write::run(&config, &index)?;
    static_copy::run(&config)?;
    Ok(())
}
