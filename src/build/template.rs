use crate::build::Output;
use crate::config::Config;
use crate::doc::Doc;
use crate::doc_index::DocIndex;
use crate::site_data::SiteData;
use crate::tera_env::build_template_env;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::sync::Arc;
use tera::Tera;

/// Render every source doc (read-only, from the frozen `classification`) and
/// every archive-emitted page into its final output, returning [`Output`]s for
/// [`write::run`](super::write::run).
///
/// The env's `collection()`/`taxonomy()`/`backlinks` functions read the frozen
/// classification (source docs only — archive pages are absent by design).
/// Rendering is read-only: source docs are never mutated, so no clone of the
/// corpus is needed; archive pages are owned and consumed here.
pub fn run(
    config: &Config,
    site_data: &SiteData,
    classification: &Arc<DocIndex>,
    archive_docs: Vec<Doc>,
) -> Result<Vec<Output>> {
    let env = build_template_env(config, classification.clone())?;

    // `Tera::render` takes `&self`, so the env is shared across Rayon workers by
    // reference. Each doc renders independently into its own output.
    let mut outputs: Vec<Output> = classification
        .par_docs()
        .map(|doc| render(&env, site_data, doc))
        .collect::<Result<Vec<_>>>()?;
    let archive_outputs: Vec<Output> = archive_docs
        .par_iter()
        .map(|doc| render(&env, site_data, doc))
        .collect::<Result<Vec<_>>>()?;
    outputs.extend(archive_outputs);
    Ok(outputs)
}

/// Render one doc to an [`Output`]. A doc with no `template` keeps its
/// markup-phase content verbatim; otherwise its template is rendered with the
/// doc as `page` (plus `pagination`/`term` for archive pages).
fn render(env: &Tera, site_data: &SiteData, doc: &Doc) -> Result<Output> {
    let Some(template_name) = doc.template.as_deref() else {
        return Ok(Output {
            output_path: doc.output_path.clone(),
            content: doc.content.clone(),
            id_path: doc.id_path.clone(),
        });
    };

    let mut ctx = tera::Context::new();
    ctx.insert("page", doc);
    ctx.insert("site", &site_data.site);
    ctx.insert("data", &site_data.data);
    if let Some(pagination) = doc.data.get("pagination") {
        ctx.insert("pagination", pagination);
    }
    if let Some(term) = doc.data.get("term") {
        ctx.insert("term", term);
    }
    let content = env.render(template_name, &ctx).with_context(|| {
        format!(
            "rendering template `{}` for {}",
            template_name,
            doc.id_path.display()
        )
    })?;
    Ok(Output {
        output_path: doc.output_path.clone(),
        content,
        id_path: doc.id_path.clone(),
    })
}
