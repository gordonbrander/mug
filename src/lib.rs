pub mod config;
pub mod doc;
pub mod frontmatter;
pub mod generate;
pub mod index;
pub mod markup;
pub mod permalink;
pub mod query;
pub mod read;
pub mod site_data;
pub mod static_copy;
pub mod template;
pub mod tera_env;
pub mod write;

use anyhow::Result;
use config::Config;
use site_data::SiteData;
use std::path::Path;

pub fn build() -> Result<()> {
    let (config, site) = Config::load(Path::new("config.yaml"))?;
    let site_data = SiteData::load(&config, site)?;
    let mut index = read::run(&config)?;
    markup::run(&config, &site_data, &mut index)?;
    generate::run(&config, &mut index)?;
    template::run(&config, &site_data, &mut index)?;
    write::run(&config, &index)?;
    static_copy::run(&config)?;
    Ok(())
}
