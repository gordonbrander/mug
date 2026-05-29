//! Tera environments used by the markup and template phases. Each phase gets
//! its own `Tera` with a curated filter/function set; the submodules under
//! `tera_env/` hold the adapters that bridge pure domain logic (`query`,
//! `backlinks`, `permalink`, `html`) to Tera's filter/function API.
//!
//! Spec §11: the markup env intentionally omits index-listing filters
//! (`query`, `backlinks`) because the index is not yet meaningful at
//! body-render time. URL filters and text filters ship on both envs — they
//! are 1:1 lookups or string composition, not listings.

mod backlinks;
mod macros;
mod query;
mod text;
mod url;

use crate::build::markup::wikilink::build_stem_index;
use crate::config::Config;
use crate::doc::{Doc, DocMeta};
use anyhow::{Context, Result};
use comrak::plugins::syntect::{SyntectAdapter, SyntectAdapterBuilder};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tera::Tera;

/// Process-wide syntect adapter. Loading the default syntax + theme sets is
/// non-trivial, so build it once and share it across every markup env (and
/// across `watch`/`serve` rebuilds) rather than rebuilding per
/// `build_markup_env` call.
static SYNTECT: LazyLock<Arc<SyntectAdapter>> = LazyLock::new(|| {
    Arc::new(SyntectAdapterBuilder::new().theme("InspiredGitHub").build())
});

/// Markup-phase Tera env paired with the auto-import preamble for
/// `templates/macros/*.html` (Phase 11) and the comrak config used to render
/// Markdown bodies. Markdown bodies can invoke macros as shortcodes without
/// writing `{% import %}` boilerplate: `markup::render` prepends
/// `macro_preamble` to every body before `render_str`. `options` and `syntect`
/// drive the comrak pass; the syntect adapter loads its syntax/theme sets once
/// here rather than per-doc.
#[derive(Clone)]
pub struct MarkupEnv {
    pub tera: Tera,
    pub macro_preamble: String,
    pub options: comrak::Options<'static>,
    pub syntect: Arc<SyntectAdapter>,
    /// Slugified file-stem → candidate docs, built once from the snapshot so
    /// wikilink resolution is a hash lookup rather than a full scan per link.
    pub stem_index: HashMap<String, Vec<DocMeta>>,
    /// Mirror of `Config::hashtags`: when true, `markup::render` runs the
    /// inline `#hashtag` extraction pass.
    pub hashtags: bool,
}

/// comrak options for the markup phase. `unsafe_` passes raw HTML through (Tera
/// macros, the wikilink rewrite, and author inline HTML all rely on it — same
/// trust model as the previous pulldown-cmark `html` feature). GFM extensions
/// are on, plus the wikilink extension whose `[[url|label]]` order matches
/// Mug's `[[Target|Display]]`.
fn markup_options() -> comrak::Options<'static> {
    let mut options = comrak::Options::default();
    options.render.r#unsafe = true;
    options.extension.wikilinks_title_after_pipe = true;
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.autolink = true;
    options
}

/// Tera environment used by the markup phase. Intentionally restricted: no
/// index-listing filters/functions (`query`, `backlinks`) are registered here,
/// because the index is not yet meaningful at body-render time (spec §11).
pub fn build_markup_env(config: &Config, docs: Arc<Vec<DocMeta>>) -> Result<MarkupEnv> {
    let mut tera = load_templates(config)?;
    // Build the stem index before `docs` is moved into the URL filters.
    let stem_index = build_stem_index(&docs);
    url::register(&mut tera, docs, config.site_url.clone(), config.base_path.clone());
    text::register(&mut tera);
    let macro_preamble = macros::discover_imports(config);
    Ok(MarkupEnv {
        tera,
        macro_preamble,
        options: markup_options(),
        syntect: SYNTECT.clone(),
        stem_index,
        hashtags: config.hashtags,
    })
}

/// Tera environment used by the template phase. The full filter set lands
/// here. `docs` is a frozen snapshot of the index taken at the start of the
/// template phase — closures registered here borrow from it for the rest of
/// the build.
pub fn build_template_env(config: &Config, docs: Arc<Vec<Doc>>) -> Result<Tera> {
    let mut env = load_templates(config)?;
    query::register(&mut env, docs.clone());
    backlinks::register(&mut env, docs.clone());
    // URL filters only ever read `id_path`/`output_path` — project to the
    // narrower `DocMeta` view to share one filter implementation with the
    // markup env.
    let url_docs: Arc<Vec<DocMeta>> = Arc::new(docs.iter().map(DocMeta::from).collect());
    url::register(
        &mut env,
        url_docs,
        config.site_url.clone(),
        config.base_path.clone(),
    );
    text::register(&mut env);
    Ok(env)
}

fn load_templates(config: &Config) -> Result<Tera> {
    if !config.templates_dir.exists() {
        return Ok(Tera::default());
    }
    let pattern = format!("{}/**/*.{{html,xml}}", config.templates_dir.display());
    Tera::new(&pattern).with_context(|| format!("loading templates from {}", pattern))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg_without_templates() -> Config {
        Config {
            templates_dir: PathBuf::from("/definitely/does/not/exist/templates"),
            ..Config::default()
        }
    }

    fn cfg_with_urls(site_url: Option<&str>, base_path: &str) -> Config {
        Config {
            templates_dir: PathBuf::from("/definitely/does/not/exist/templates"),
            site_url: site_url.map(String::from),
            base_path: base_path.to_string(),
            ..Config::default()
        }
    }

    fn empty_snapshot() -> Arc<Vec<Doc>> {
        Arc::new(Vec::new())
    }

    fn empty_meta_snapshot() -> Arc<Vec<DocMeta>> {
        Arc::new(Vec::new())
    }

    #[test]
    fn markup_env_does_not_register_query() {
        // Guards spec §11: `query` must never be callable from the markup env.
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.tera.render_str("{{ query() }}", &ctx).is_err());
    }

    #[test]
    fn markup_env_does_not_register_backlinks() {
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.tera.render_str("{{ backlinks() }}", &ctx).is_err());
    }

    #[test]
    fn markup_env_does_not_register_backlinks_as_filter() {
        // Spec §11: even the filter form must fail on the markup env.
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.tera.render_str("{{ 'a.md' | backlinks }}", &ctx).is_err());
    }

    #[test]
    fn missing_templates_dir_is_not_an_error() {
        // Fixtures 01 and 02 have no templates/ dir; the env must still build.
        let env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        assert!(env.macro_preamble.is_empty());
        assert!(build_template_env(&cfg_without_templates(), empty_snapshot()).is_ok());
    }

    #[test]
    fn template_env_registers_query() {
        let env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let mut env = env;
        let out = env
            .render_str("{{ query() | length }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "0");
    }

    #[test]
    fn template_env_registers_backlinks() {
        let mut env =
            build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let out = env
            .render_str(
                "{{ 'missing.md' | backlinks | length }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "0");
    }

    fn one_doc_snapshot() -> Arc<Vec<Doc>> {
        let mut d = Doc::default();
        d.id_path = PathBuf::from("posts/hello.md");
        d.output_path = PathBuf::from("posts/hello.html");
        Arc::new(vec![d])
    }

    fn one_doc_meta_snapshot() -> Arc<Vec<DocMeta>> {
        Arc::new(one_doc_snapshot().iter().map(DocMeta::from).collect())
    }

    #[test]
    fn markup_env_registers_permalink() {
        let mut env =
            build_markup_env(&cfg_without_templates(), one_doc_meta_snapshot()).unwrap();
        let out = env
            .tera
            .render_str(
                "{{ 'posts/hello.md' | permalink }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "/posts/hello.html");
    }

    #[test]
    fn template_env_registers_permalink() {
        let mut env =
            build_template_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
        let out = env
            .render_str(
                "{{ 'posts/hello.md' | permalink }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "/posts/hello.html");
    }

    #[test]
    fn permalink_filter_errors_on_unknown_id_path() {
        let mut env =
            build_markup_env(&cfg_without_templates(), one_doc_meta_snapshot()).unwrap();
        let res = env
            .tera
            .render_str("{{ 'missing.md' | permalink }}", &tera::Context::new());
        assert!(res.is_err());
    }

    #[test]
    fn permalink_with_site_url_and_base_path_is_absolute() {
        let mut env = build_template_env(
            &cfg_with_urls(Some("https://example.com"), "/blog"),
            one_doc_snapshot(),
        )
        .unwrap();
        let out = env
            .render_str(
                "{{ 'posts/hello.md' | permalink }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "https://example.com/blog/posts/hello.html");
    }

    #[test]
    fn permalink_without_site_url_falls_back_to_root_relative() {
        let mut env =
            build_template_env(&cfg_with_urls(None, "/blog"), one_doc_snapshot())
                .unwrap();
        let out = env
            .render_str(
                "{{ 'posts/hello.md' | permalink }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "/blog/posts/hello.html");
    }

    #[test]
    fn permalink_no_config_preserves_root_relative_output() {
        // Phase 9 regression: no site_url, no base_path → identical to pre-12 behavior.
        let mut env =
            build_template_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
        let out = env
            .render_str(
                "{{ 'posts/hello.md' | permalink }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "/posts/hello.html");
    }

    #[test]
    fn link_filter_returns_root_relative_with_base_path() {
        let mut env = build_template_env(
            &cfg_with_urls(Some("https://example.com"), "/blog"),
            one_doc_snapshot(),
        )
        .unwrap();
        let out = env
            .render_str("{{ 'posts/hello.md' | link }}", &tera::Context::new())
            .unwrap();
        // `link` ignores site_url entirely.
        assert_eq!(out, "/blog/posts/hello.html");
    }

    #[test]
    fn link_filter_errors_on_unknown_id_path() {
        let mut env =
            build_template_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
        assert!(
            env.render_str("{{ 'missing.md' | link }}", &tera::Context::new())
                .is_err()
        );
    }

    #[test]
    fn relative_url_prepends_base_path() {
        let mut env =
            build_template_env(&cfg_with_urls(None, "/blog"), one_doc_snapshot())
                .unwrap();
        let out = env
            .render_str("{{ 'css/site.css' | relative_url }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "/blog/css/site.css");
    }

    #[test]
    fn relative_url_handles_leading_slash_in_input() {
        let mut env =
            build_template_env(&cfg_with_urls(None, "/blog"), one_doc_snapshot())
                .unwrap();
        let out = env
            .render_str("{{ '/css/site.css' | relative_url }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "/blog/css/site.css");
    }

    #[test]
    fn relative_url_with_empty_base_path() {
        let mut env =
            build_template_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
        let out = env
            .render_str("{{ 'css/site.css' | relative_url }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "/css/site.css");
    }

    #[test]
    fn absolute_url_prepends_site_url_and_base_path() {
        let mut env = build_template_env(
            &cfg_with_urls(Some("https://example.com"), "/blog"),
            one_doc_snapshot(),
        )
        .unwrap();
        let out = env
            .render_str("{{ 'feed.xml' | absolute_url }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "https://example.com/blog/feed.xml");
    }

    #[test]
    fn absolute_url_falls_back_without_site_url() {
        let mut env =
            build_template_env(&cfg_with_urls(None, "/blog"), one_doc_snapshot())
                .unwrap();
        let out = env
            .render_str("{{ 'feed.xml' | absolute_url }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "/blog/feed.xml");
    }

    #[test]
    fn markup_env_registers_all_four_url_filters() {
        let mut env = build_markup_env(
            &cfg_with_urls(Some("https://example.com"), "/blog"),
            one_doc_meta_snapshot(),
        )
        .unwrap();
        for body in [
            "{{ 'posts/hello.md' | permalink }}",
            "{{ 'posts/hello.md' | link }}",
            "{{ 'css/x' | relative_url }}",
            "{{ 'css/x' | absolute_url }}",
        ] {
            assert!(
                env.tera.render_str(body, &tera::Context::new()).is_ok(),
                "markup env missing filter for body: {}",
                body,
            );
        }
    }

    /// Small RAII temp-dir helper for macro-discovery tests. Removed on drop
    /// so test failures don't leak directories in /tmp.
    struct TempTemplatesDir(PathBuf);
    impl TempTemplatesDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "mug-tera_env-test-{}-{}",
                name,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn cfg(&self) -> Config {
            Config {
                templates_dir: self.0.clone(),
                ..Config::default()
            }
        }
        fn write_macros_file(&self, name: &str, body: &str) {
            let macros = self.0.join("macros");
            std::fs::create_dir_all(&macros).unwrap();
            std::fs::write(macros.join(name), body).unwrap();
        }
    }
    impl Drop for TempTemplatesDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn markup_env_imports_macros_from_macros_dir() {
        let tmp = TempTemplatesDir::new("import");
        tmp.write_macros_file(
            "youtube.html",
            "{% macro embed(id) %}<iframe src=\"https://youtube.com/embed/{{ id }}\"></iframe>{% endmacro embed %}",
        );
        let mut env = build_markup_env(&tmp.cfg(), empty_meta_snapshot()).unwrap();
        // `markup::render` prepends the preamble before calling render_str; do
        // the same here so this test exercises the end-to-end contract.
        let body = format!(
            "{}{{{{ youtube::embed(id=\"abc\") }}}}",
            env.macro_preamble
        );
        let out = env
            .tera
            .render_str(&body, &tera::Context::new())
            .unwrap();
        assert_eq!(out, "<iframe src=\"https://youtube.com/embed/abc\"></iframe>");
    }

    #[test]
    fn markup_env_handles_missing_macros_dir() {
        let tmp = TempTemplatesDir::new("no-macros");
        // templates/ exists, templates/macros/ does not.
        let env = build_markup_env(&tmp.cfg(), empty_meta_snapshot()).unwrap();
        assert!(env.macro_preamble.is_empty());
    }

    #[test]
    fn markup_env_handles_empty_macros_dir() {
        let tmp = TempTemplatesDir::new("empty-macros");
        std::fs::create_dir_all(tmp.0.join("macros")).unwrap();
        let env = build_markup_env(&tmp.cfg(), empty_meta_snapshot()).unwrap();
        assert!(env.macro_preamble.is_empty());
    }

    #[test]
    fn markup_env_macro_preamble_is_deterministic() {
        let tmp = TempTemplatesDir::new("sorted");
        // Write in reverse order to prove sort is by stem, not FS read order.
        tmp.write_macros_file("twitter.html", "{% macro x() %}t{% endmacro x %}");
        tmp.write_macros_file("aside.html", "{% macro x() %}a{% endmacro x %}");
        let env = build_markup_env(&tmp.cfg(), empty_meta_snapshot()).unwrap();
        assert_eq!(
            env.macro_preamble,
            "{% import \"macros/aside.html\" as aside %}\n\
             {% import \"macros/twitter.html\" as twitter %}\n"
        );
    }

    #[test]
    fn markup_env_ignores_non_html_files_in_macros_dir() {
        let tmp = TempTemplatesDir::new("non-html");
        tmp.write_macros_file("notes.md", "not a macro");
        tmp.write_macros_file("real.html", "{% macro hi() %}hi{% endmacro hi %}");
        let env = build_markup_env(&tmp.cfg(), empty_meta_snapshot()).unwrap();
        assert_eq!(
            env.macro_preamble,
            "{% import \"macros/real.html\" as real %}\n"
        );
    }

    #[test]
    fn truncate_words_filter_on_markup_env() {
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let out = env
            .tera
            .render_str(
                "{{ 'hello world foo bar baz' | truncate_words(length=15) }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "hello world…");
    }

    #[test]
    fn truncate_words_filter_on_template_env() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let out = env
            .render_str(
                "{{ 'hello world foo bar baz' | truncate_words(length=15) }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "hello world…");
    }

    #[test]
    fn truncate_words_filter_default_length_is_250() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        // 100 chars — well under 250 — should pass through unchanged.
        let short = "a".repeat(100);
        let body = format!("{{{{ '{}' | truncate_words }}}}", short);
        let out = env.render_str(&body, &tera::Context::new()).unwrap();
        assert_eq!(out, short);
    }

    #[test]
    fn truncate_words_composes_with_striptags() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let out = env
            .render_str(
                "{{ '<p>hello world foo bar baz</p>' | striptags | truncate_words(length=15) }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "hello world…");
    }

    #[test]
    fn template_env_does_not_auto_import_macros() {
        // The template env build does not surface a preamble, and a body that
        // calls a macro without an explicit `{% import %}` must fail.
        let tmp = TempTemplatesDir::new("template-no-auto");
        tmp.write_macros_file(
            "youtube.html",
            "{% macro embed(id) %}x{% endmacro embed %}",
        );
        let mut env = build_template_env(&tmp.cfg(), empty_snapshot()).unwrap();
        assert!(
            env.render_str("{{ youtube::embed(id=\"a\") }}", &tera::Context::new())
                .is_err()
        );
    }
}
