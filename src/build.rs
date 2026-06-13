//! The site build pipeline. Stages run in order, each consuming the index the
//! previous stage left behind. `║` marks the embarrassingly parallel (Rayon)
//! stages; the rest are sequential:
//!
//! 1. [`read`] — scan `content/` into a [`DocIndex`](crate::doc_index::DocIndex).
//!    Docs with `draft: true` frontmatter are dropped here unless `include_drafts`
//!    (local `serve`/`watch`, or `italic build --drafts`), so they stay out of every
//!    later stage.
//! 2. [`classify::collections`] — evaluate collection membership (frontmatter
//!    only; pre-markup so defaults can fill members).
//! 3. [`defaults`] — fill each collection's members with its default frontmatter.
//! 4. [`markup`] ║ — render markdown bodies through Tera + comrak (the hashtag
//!    pass mutates `terms`).
//! 5. [`classify::taxonomies`] — bucket taxonomy terms (post-markup). The index
//!    is now fully classified; it is frozen into an `Arc` for the rest.
//! 6. [`archive`] ║ — run `archives/` over the frozen index, returning view pages
//!    (never re-classified).
//! 7. [`template`] ║ — render each source doc (read-only) and archive page into
//!    final output, producing [`Output`]s.
//! 8. [`alias`] ║ — emit a client-side redirect stub for each doc `aliases:`
//!    entry, appended to the outputs (generated, never classified).
//! 9. [`write`] — write the outputs to `output_dir`, skipping path collisions
//!    first-writer-wins (so a real page is never clobbered by an alias stub).
//! 10. [`static_copy`] — copy `static/` over the top.
//!
//! Collections are pure frontmatter metadata, so they classify before markup;
//! taxonomies depend on the markup-phase hashtag pass, so they classify after.
//! Once the index is frozen (step 5) it is never mutated again: archives and the
//! template phase read it by shared `Arc` reference and write their output to the
//! side, so there is no corpus-wide clone.

pub mod alias;
pub mod archive;
pub mod classify;
pub mod defaults;
pub mod markup;
pub mod read;
pub mod static_copy;
pub mod template;
pub mod write;

use crate::config::Config;
use crate::site_data::SiteData;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One file to write: its `output_path` (relative to `output_dir`), the rendered
/// `content`, and the `id_path` of the doc it came from. The provenance lets
/// [`write::run`] name the culprit when two outputs collide on one path.
pub struct Output {
    pub output_path: PathBuf,
    pub content: String,
    pub id_path: PathBuf,
}

pub fn run(include_drafts: bool) -> Result<()> {
    let (config, site) = Config::load_with_theme(Path::new("config.yaml"))?;
    let site_data = SiteData::load(&config, site)?;
    let mut index = read::run(&config, include_drafts)?;
    // Collections classify from frontmatter (pre-markup).
    classify::collections(&config, &mut index);
    // Defaults filled in for collection members before bodies render
    defaults::run(&config, &mut index)?;
    markup::run(&config, &site_data, &mut index)?;
    // Taxonomies classify after markup (hashtags mutate terms); backlinks invert
    // the markup-populated `doc.links`. The index is now complete — freeze it
    // once and share it by `Arc`.
    classify::taxonomies(&config, &mut index);
    classify::backlinks(&mut index);
    let index = Arc::new(index);
    // Archives read the frozen index and emit view pages (not classified).
    let archive_docs = archive::run(&config, &site_data, &index)?;
    // Render source docs (read-only) + archive pages into output. Alias redirect
    // stubs are appended after, so a real page always wins a path collision
    // (write::run is first-writer-wins).
    let mut outputs = template::run(&config, &site_data, &index, archive_docs)?;
    outputs.extend(alias::run(&config, &index)?);
    write::run(&config, &outputs)?;
    static_copy::run(&config)?;
    Ok(())
}
