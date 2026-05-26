use crate::config::Config;
use crate::doc::DocKind;
use crate::index::Index;
use crate::tera_env::build_markup_env;
use anyhow::{Context, Result};
use pulldown_cmark::{Parser, html};

pub fn run(config: &Config, index: &mut Index) -> Result<()> {
    let mut markup_env = build_markup_env(config)?;

    for doc in &mut index.docs {
        let mut ctx = tera::Context::new();
        ctx.insert("doc", &*doc);

        // Tera over the body string runs unescaped (the one-off template has
        // no file extension, so HTML autoescape is off). Authors writing
        // `{{ ... }}` inside Markdown code fences should wrap them in
        // `{% raw %}` per spec §6.
        let rendered = markup_env
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
    }
    Ok(())
}
