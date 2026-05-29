pub(crate) mod wikilink;

use crate::config::Config;
use crate::doc::{Doc, DocKind, DocMeta};
use crate::html as html_utils;
use crate::index::Index;
use crate::site_data::SiteData;
use crate::tera_env::{MarkupEnv, build_markup_env};
use anyhow::{Context, Result};
use std::borrow::Cow;
use std::sync::Arc;

const SUMMARY_MAX_CHARS: usize = 250;

/// Run the markup phase over `doc`. Renders the body string through the
/// restricted Tera env, then for Markdown docs parses with comrak, resolves
/// wikilinks on the AST (populating `doc.links`), and renders to HTML.
/// Wikilink target resolution uses `env.stem_index`, the frozen stem→candidate
/// grouping built when the env was constructed. Exposed publicly so
/// `generate::run` can apply the same transformation to its emitted docs.
pub fn render(env: &mut MarkupEnv, site_data: &SiteData, doc: &mut Doc) -> Result<()> {
    let mut ctx = tera::Context::new();
    ctx.insert("page", &*doc);
    ctx.insert("site", &site_data.site);
    ctx.insert("data", &site_data.data);
    if let Some(pagination) = doc.data.get("pagination") {
        ctx.insert("pagination", pagination);
    }

    // Tera over the body string runs unescaped (the one-off template has
    // no file extension, so HTML autoescape is off). Authors writing
    // `{{ ... }}` inside Markdown code fences should wrap them in
    // `{% raw %}` per spec §6.
    // The macro preamble auto-imports `templates/macros/*.html` so Markdown
    // bodies can call them as shortcodes (Phase 11). Empty when no macros
    // exist — skip the allocation in that case.
    let body_with_imports: Cow<str> = if env.macro_preamble.is_empty() {
        Cow::Borrowed(doc.content.as_str())
    } else {
        Cow::Owned(format!("{}{}", env.macro_preamble, doc.content))
    };
    let rendered = env
        .tera
        .render_str(&body_with_imports, &ctx)
        .with_context(|| format!("markup-phase Tera in {}", doc.id_path.display()))?;

    // Markdown bodies are parsed by comrak; wikilinks are resolved on the AST
    // (spec §8), so they are code-span aware — `[[…]]` inside a fence stays
    // literal. `rendered` (post-Tera) is parsed directly; Raw/Yaml bodies pass
    // through untouched. `env.stem_index` is the frozen stem→candidate grouping
    // used by wikilink resolution.
    doc.content = match doc.kind() {
        DocKind::Markdown => {
            let arena = comrak::Arena::new();
            let root = comrak::parse_document(&arena, &rendered, &env.options);
            doc.links = wikilink::resolve_in_ast(root, doc, &env.stem_index);
            let mut plugins = comrak::options::Plugins::default();
            plugins.render.codefence_syntax_highlighter = Some(env.syntect.as_ref());
            let mut out = String::new();
            comrak::format_html_with_plugins(root, &env.options, &mut out, &plugins)
                .with_context(|| format!("comrak render in {}", doc.id_path.display()))?;
            out
        }
        DocKind::Raw | DocKind::Yaml => rendered,
    };

    // Fallback summary: only for Markdown docs whose frontmatter didn't
    // supply one. Strips HTML from the just-rendered body and truncates at a
    // word boundary.
    if doc.kind() == DocKind::Markdown && doc.summary.is_empty() {
        let stripped = html_utils::strip_tags(&doc.content);
        doc.summary = html_utils::truncate_words(&stripped, SUMMARY_MAX_CHARS);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_yaml_ng::Mapping;
    use std::path::PathBuf;

    fn cfg() -> Config {
        Config {
            templates_dir: PathBuf::from("/definitely/does/not/exist/templates"),
            ..Config::default()
        }
    }

    fn site_data() -> SiteData {
        SiteData {
            site: Mapping::new(),
            data: Mapping::new(),
        }
    }

    fn render_doc(doc: &mut Doc) {
        let snapshot: Arc<Vec<DocMeta>> = Arc::new(Vec::new());
        let mut env = build_markup_env(&cfg(), snapshot).unwrap();
        render(&mut env, &site_data(), doc).unwrap();
    }

    #[test]
    fn markdown_doc_gets_fallback_summary() {
        let mut doc = Doc::new(
            PathBuf::from("post.md"),
            "# Hello\n\nThis is the body of the post. It has several words.".to_string(),
            Mapping::new(),
        );
        render_doc(&mut doc);
        assert!(!doc.summary.is_empty(), "expected a fallback summary");
        // No HTML tags should leak into the summary.
        assert!(!doc.summary.contains('<'));
        assert!(!doc.summary.contains('>'));
        assert!(doc.summary.contains("This is the body"));
    }

    #[test]
    fn fallback_summary_truncates_long_bodies() {
        let long_body: String = "word ".repeat(100); // 500 chars
        let mut doc = Doc::new(PathBuf::from("post.md"), long_body, Mapping::new());
        render_doc(&mut doc);
        assert!(doc.summary.chars().count() <= 250);
        assert!(doc.summary.ends_with('…'));
    }

    #[test]
    fn frontmatter_summary_is_preserved_verbatim() {
        let mut data = Mapping::new();
        data.insert(
            serde_yaml_ng::Value::String("summary".into()),
            serde_yaml_ng::Value::String("Hand-written blurb.".into()),
        );
        let mut doc = Doc::new(
            PathBuf::from("post.md"),
            "# Hello\n\nMuch longer body that would otherwise yield a different summary."
                .to_string(),
            data,
        );
        render_doc(&mut doc);
        assert_eq!(doc.summary, "Hand-written blurb.");
    }

    #[test]
    fn raw_doc_without_frontmatter_summary_stays_empty() {
        let mut doc = Doc::new(
            PathBuf::from("page.html"),
            "<p>raw html body</p>".to_string(),
            Mapping::new(),
        );
        render_doc(&mut doc);
        assert_eq!(doc.summary, "");
    }

    #[test]
    fn yaml_doc_without_frontmatter_summary_stays_empty() {
        let mut doc = Doc::new(
            PathBuf::from("page.yaml"),
            "<p>yaml-derived body</p>".to_string(),
            Mapping::new(),
        );
        render_doc(&mut doc);
        assert_eq!(doc.summary, "");
    }
}

pub fn run(config: &Config, site_data: &SiteData, index: &mut Index) -> Result<()> {
    // Frozen `DocMeta` view of the index for wikilink resolution and the
    // URL filters. The projection drops `content`/`links`/`data`/
    // `template`, so the type system enforces that the markup phase can't
    // read another doc's body or stale markup-phase state.
    let snapshot: Arc<Vec<DocMeta>> =
        Arc::new(index.docs.iter().map(DocMeta::from).collect());
    let mut env = build_markup_env(config, snapshot.clone())?;
    for doc in &mut index.docs {
        render(&mut env, site_data, doc)?;
    }
    Ok(())
}
