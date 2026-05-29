use crate::config::Config;
use crate::index::Index;
use crate::site_data::SiteData;
use crate::tera_env::build_template_env;
use anyhow::{Context, Result};
use std::sync::Arc;

pub fn run(config: &Config, site_data: &SiteData, index: &mut Index) -> Result<()> {
    // Snapshot the index for the `query` function. By spec §11, the index is
    // fully populated before this phase runs, so a frozen view is exactly what
    // every template sees.
    let snapshot = Arc::new(index.docs.clone());
    let env = build_template_env(config, snapshot)?;

    for doc in &mut index.docs {
        let Some(template_name) = doc.template.clone() else {
            continue;
        };

        let mut ctx = tera::Context::new();
        ctx.insert("page", &*doc);
        ctx.insert("site", &site_data.site);
        ctx.insert("data", &site_data.data);
        if let Some(pagination) = doc.data.get("pagination") {
            ctx.insert("pagination", pagination);
        }

        doc.content = env.render(&template_name, &ctx).with_context(|| {
            format!(
                "rendering template `{}` for {}",
                template_name,
                doc.id_path.display()
            )
        })?;
    }
    Ok(())
}
