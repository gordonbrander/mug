use crate::config::Config;
use crate::index::Index;
use crate::site_data::SiteData;
use crate::tera_env::build_template_env;
use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Serialize)]
struct Page<'a> {
    content: &'a str,
}

pub fn run(config: &Config, site_data: &SiteData, index: &mut Index) -> Result<()> {
    let env = build_template_env(config)?;

    for doc in &mut index.docs {
        let Some(template_name) = doc.template.clone() else {
            continue;
        };

        let mut ctx = tera::Context::new();
        ctx.insert("doc", &*doc);
        ctx.insert(
            "page",
            &Page {
                content: &doc.content,
            },
        );
        ctx.insert("site", &site_data.site);
        ctx.insert("data", &site_data.data);

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
