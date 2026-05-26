pub mod config;
pub mod doc;
pub mod frontmatter;
pub mod generate;
pub mod index;
pub mod markup;
pub mod read;
pub mod template;
pub mod tera_env;
pub mod write;

use anyhow::Result;
use config::Config;

pub fn build() -> Result<()> {
    let config = Config::default();
    let mut index = read::run(&config)?;
    markup::run(&config, &mut index)?;
    generate::run(&config, &mut index)?;
    template::run(&config, &mut index)?;
    write::run(&config, &index)?;
    Ok(())
}
