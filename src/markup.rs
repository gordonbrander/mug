use crate::config::Config;
use crate::doc::{Doc, DocKind};
use crate::index::Index;
use crate::site_data::SiteData;
use crate::tera_env::build_markup_env;
use crate::wikilink;
use anyhow::{Context, Result};
use pulldown_cmark::{Parser, html};
use std::sync::Arc;
use tera::Tera;

/// Run the markup phase over `doc`. Renders the body string through the
/// restricted Tera env, expands wikilinks (Markdown only), and for `.md` docs
/// runs the result through pulldown-cmark. `snapshot` is the frozen index view
/// used by `wikilink::expand` for target resolution. Exposed publicly so
/// `generate::run` can apply the same transformation to its emitted docs.
pub fn render(
    env: &mut Tera,
    site_data: &SiteData,
    doc: &mut Doc,
    snapshot: &Arc<Vec<Doc>>,
) -> Result<()> {
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

    // Wikilink expansion happens between Tera and Markdown render, for
    // Markdown docs only (spec §8). Resolution targets are read from
    // `snapshot` (frozen index view).
    let after_wikilinks = if doc.kind() == DocKind::Markdown {
        let (expanded, outlinks) = wikilink::expand(&rendered, doc, snapshot.as_slice());
        doc.outlinks = outlinks;
        expanded
    } else {
        rendered
    };

    doc.content = match doc.kind() {
        DocKind::Markdown => {
            let parser = Parser::new(&after_wikilinks);
            let mut html_out = String::new();
            html::push_html(&mut html_out, parser);
            html_out
        }
        DocKind::Html | DocKind::Yaml => after_wikilinks,
    };
    Ok(())
}

pub fn run(config: &Config, site_data: &SiteData, index: &mut Index) -> Result<()> {
    // Snapshot the index for wikilink resolution and the `permalink` filter.
    // `id_path` and `output_path` are final by the end of `read::run`; the
    // snapshot's stale `content`/`outlinks` are unobserved (see plan §
    // "Index lifecycle").
    let snapshot = Arc::new(index.docs.clone());
    let mut env = build_markup_env(config, snapshot.clone())?;
    for doc in &mut index.docs {
        render(&mut env, site_data, doc, &snapshot)?;
    }
    Ok(())
}
