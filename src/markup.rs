use crate::config::Config;
use crate::doc::{Doc, DocKind};
use crate::index::Index;
use crate::site_data::SiteData;
use crate::tera_env::build_markup_env;
use anyhow::{Context, Result};
use pulldown_cmark::{Parser, html};
use tera::Tera;

/// Run the markup phase over `doc`. Renders the body string through the
/// restricted Tera env and, for `.md` docs, runs the result through
/// pulldown-cmark. Exposed publicly so `generate::run` can apply the same
/// transformation to its emitted virtual docs.
pub fn render(env: &mut Tera, site_data: &SiteData, doc: &mut Doc) -> Result<()> {
    let mut ctx = tera::Context::new();
    ctx.insert("doc", &*doc);
    ctx.insert("site", &site_data.site);
    ctx.insert("data", &site_data.data);
    if let Some(pagination) = doc.data.get("pagination") {
        ctx.insert("pagination", pagination);
    }

    // Tera over the body string runs unescaped (the one-off template has
    // no file extension, so HTML autoescape is off). Authors writing
    // `{{ ... }}` inside Markdown code fences should wrap them in
    // `{% raw %}` per spec §6.
    let rendered = env
        .render_str(&doc.content, &ctx)
        .with_context(|| format!("markup-phase Tera in {}", doc.id_path.display()))?;

    doc.content = match doc.kind() {
        DocKind::Markdown => {
            let parser = Parser::new(&rendered);
            let mut html_out = String::new();
            html::push_html(&mut html_out, parser);
            html_out
        }
        DocKind::Html | DocKind::Yaml => rendered,
    };
    Ok(())
}

pub fn run(config: &Config, site_data: &SiteData, index: &mut Index) -> Result<()> {
    let mut env = build_markup_env(config)?;
    for doc in &mut index.docs {
        render(&mut env, site_data, doc)?;
    }
    Ok(())
}
