//! Tera environments used by the markup and template phases. Each phase gets
//! its own `Tera` with a curated filter/function set; the submodules under
//! `tera_env/` hold the adapters that bridge pure domain logic (`query`,
//! `backlinks`, `permalink`, `html`) to Tera's filter/function API.
//!
//! Spec §11: the markup env intentionally omits index-listing functions
//! (`collection`, `taxonomy`, `backlinks`) because the index is not yet
//! meaningful at body-render time. URL filters and text filters ship on both
//! envs — they are 1:1 lookups or string composition, not listings.

mod backlinks;
mod collection;
mod doc;
mod entries;
mod macros;
mod markdown;
mod taxonomy;
mod text;
mod url;

use crate::build::markup::wikilink::build_stem_index;
use crate::config::{self, Config};
use crate::doc::DocMeta;
use crate::doc_index::DocIndex;
use anyhow::{Context, Result};
use comrak::plugins::syntect::{SyntectAdapter, SyntectAdapterBuilder};
use std::collections::HashMap;
use std::path::PathBuf;
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
    let (mut tera, template_names) = load_templates(config)?;
    // Build the stem index before `docs` is moved into the URL filters.
    let stem_index = build_stem_index(&docs);
    let options = markup_options();
    url::register(
        &mut tera,
        url::lookup_from_metas(docs),
        config.site_url.clone(),
        config.base_path.clone(),
    );
    text::register(&mut tera);
    entries::register(&mut tera);
    markdown::register(&mut tera, options.clone(), SYNTECT.clone());
    let macro_preamble = macros::macro_preamble(&template_names);
    Ok(MarkupEnv {
        tera,
        macro_preamble,
        options,
        syntect: SYNTECT.clone(),
        stem_index,
        hashtags: config.hashtags,
    })
}

/// Tera environment used by the template phase. The full function set lands
/// here. `index` is a frozen [`DocIndex`] snapshot taken at the start of the
/// template phase (with collections and taxonomies already defined) — every
/// adapter shares the same `Arc`, so there is no per-function clone of the docs.
pub fn build_template_env(config: &Config, index: Arc<DocIndex>) -> Result<Tera> {
    let (mut env, _template_names) = load_templates(config)?;
    collection::register(&mut env, index.clone());
    doc::register(&mut env, index.clone());
    taxonomy::register(&mut env, index.clone());
    backlinks::register(&mut env, index.clone());
    // URL filters resolve `id_path` -> URL through the shared `DocIndex` (O(1)),
    // not a cloned `DocMeta` vec.
    url::register(
        &mut env,
        url::lookup_from_index(index),
        config.site_url.clone(),
        config.base_path.clone(),
    );
    text::register(&mut env);
    entries::register(&mut env);
    markdown::register(&mut env, markup_options(), SYNTECT.clone());
    Ok(env)
}

/// Build the Tera env by overlaying every `config.template_roots()` in order:
/// the theme's `templates/` first, the site's last, so a site template overrides
/// a theme template of the same name. Each `.html`/`.xml` file is registered
/// under its path relative to its root (`post.html`, `macros/youtube.html`) — so
/// `{% extends "base.html" %}` resolves to the site's `base.html` when the site
/// overrides it. Returns the `Tera` env alongside the registered template names
/// (in overlay order, site-wins-deduped) so the markup env can derive its macro
/// preamble from the same set (see [`macros::macro_preamble`]). With no template
/// files at all (zero-config sites, fixtures 01/02) this yields an empty `Tera`.
fn load_templates(config: &Config) -> Result<(Tera, Vec<String>)> {
    // `overlay_files` walks the roots and dedups per relative path (site wins);
    // map each surviving file to its `/`-joined Tera name, paired with its path.
    let files: Vec<(String, PathBuf)> =
        config::overlay_files(&config.template_roots(), is_html_or_xml)?
            .into_iter()
            .map(|(rel, path)| (rel_template_name(&rel), path))
            .collect();
    if files.is_empty() {
        return Ok((Tera::default(), Vec::new()));
    }
    let mut tera = Tera::default();
    let pairs: Vec<(PathBuf, Option<&str>)> =
        files.iter().map(|(name, path)| (path.clone(), Some(name.as_str()))).collect();
    tera.add_template_files(pairs)
        .context("loading templates")?;
    let names = files.into_iter().map(|(name, _)| name).collect();
    Ok((tera, names))
}

/// Whether `path` is a template file Tera should load (`.html`/`.xml`).
fn is_html_or_xml(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("html") | Some("xml")
    )
}

/// A template's Tera name: its path relative to its root, joined with `/` on
/// every platform (not the OS separator) so theme/site names match cross-platform
/// and line up with `macros/<stem>.html` references in the macro preamble.
fn rel_template_name(rel: &std::path::Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;
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

    fn empty_snapshot() -> Arc<DocIndex> {
        Arc::new(DocIndex::new())
    }

    fn empty_meta_snapshot() -> Arc<Vec<DocMeta>> {
        Arc::new(Vec::new())
    }

    #[test]
    fn markup_env_does_not_register_collection() {
        // Guards spec §11: `collection` must never be callable from the markup env.
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(
            env.tera
                .render_str("{{ collection(name=\"posts\") }}", &ctx)
                .is_err()
        );
    }

    #[test]
    fn markup_env_does_not_register_taxonomy() {
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(
            env.tera
                .render_str("{{ taxonomy(name=\"tags\") }}", &ctx)
                .is_err()
        );
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
    fn template_env_registers_collection() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        // Unknown/empty collection renders an empty list (no error).
        let out = env
            .render_str("{{ collection(name=\"posts\") | length }}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "0");
    }

    #[test]
    fn template_env_registers_doc() {
        let mut env = build_template_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
        let out = env
            .render_str(
                "{% set d = doc(id_path=\"posts/hello.md\") %}{{ d.output_path }}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "posts/hello.html");
    }

    #[test]
    fn doc_unknown_id_path_renders_null_not_error() {
        // Unknown id_path yields `null`, so `{% if doc(...) %}` guards cleanly.
        let mut env = build_template_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
        let out = env
            .render_str(
                "{% set d = doc(id_path=\"missing.md\") %}{% if d %}yes{% else %}no{% endif %}",
                &tera::Context::new(),
            )
            .unwrap();
        assert_eq!(out, "no");
    }

    #[test]
    fn doc_missing_id_path_argument_is_an_error() {
        let mut env = build_template_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
        assert!(env.render_str("{{ doc() }}", &tera::Context::new()).is_err());
    }

    #[test]
    fn markup_env_does_not_register_doc() {
        // `doc` needs the full frozen index, which the markup env lacks (spec §11).
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(
            env.tera
                .render_str("{{ doc(id_path=\"posts/hello.md\") }}", &ctx)
                .is_err()
        );
    }

    #[test]
    fn template_env_registers_taxonomy() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        // Unknown taxonomy renders an empty map (no error).
        let out = env
            .render_str("{{ taxonomy(name=\"tags\") | length }}", &tera::Context::new())
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

    fn one_doc() -> Doc {
        Doc {
            id_path: PathBuf::from("posts/hello.md"),
            output_path: PathBuf::from("posts/hello.html"),
            ..Default::default()
        }
    }

    fn one_doc_snapshot() -> Arc<DocIndex> {
        let mut idx = DocIndex::new();
        idx.insert(one_doc());
        Arc::new(idx)
    }

    fn one_doc_meta_snapshot() -> Arc<Vec<DocMeta>> {
        Arc::new(vec![DocMeta::from(&one_doc())])
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

    /// RAII temp dir holding a `theme/` and `site/` next to each other, wired into
    /// a `Config` whose theme overlays its `templates/` beneath the site's. Used
    /// to exercise the template/macro overlay (site wins per-path).
    struct TempOverlay(PathBuf);
    impl TempOverlay {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("mug-tera_env-overlay-{}-{}", name, std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        /// Write `templates/<rel>` under the theme (`layer = "theme"`) or the site
        /// (`layer = "site"`).
        fn write_template(&self, layer: &str, rel: &str, body: &str) {
            let path = self.0.join(layer).join("templates").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        fn cfg(&self) -> Config {
            Config {
                templates_dir: self.0.join("site").join("templates"),
                theme: Some(self.0.join("theme")),
                ..Config::default()
            }
        }
    }
    impl Drop for TempOverlay {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn site_template_overrides_theme_template_of_same_name() {
        let o = TempOverlay::new("override");
        o.write_template("theme", "base.html", "THEME");
        o.write_template("site", "base.html", "SITE");
        let mut env = build_template_env(&o.cfg(), empty_snapshot()).unwrap();
        let out = env
            .render_str("{% include \"base.html\" %}", &tera::Context::new())
            .unwrap();
        assert_eq!(out, "SITE");
    }

    #[test]
    fn theme_only_template_falls_through_when_site_lacks_it() {
        let o = TempOverlay::new("fallthrough");
        // Site overrides base.html but provides no post.html; the theme's
        // post.html still loads and extends the *site's* base.html.
        o.write_template("theme", "base.html", "[THEME {% block body %}{% endblock %}]");
        o.write_template("site", "base.html", "[SITE {% block body %}{% endblock %}]");
        o.write_template(
            "theme",
            "post.html",
            "{% extends \"base.html\" %}{% block body %}hi{% endblock %}",
        );
        let env = build_template_env(&o.cfg(), empty_snapshot()).unwrap();
        let out = env
            .render("post.html", &tera::Context::new())
            .unwrap();
        // Theme's post.html rendered, wrapped by the site's overriding base.html.
        assert_eq!(out, "[SITE hi]");
    }

    #[test]
    fn site_macro_overrides_theme_macro_of_same_stem() {
        let o = TempOverlay::new("macro-override");
        o.write_template(
            "theme",
            "macros/note.html",
            "{% macro show() %}THEME{% endmacro show %}",
        );
        o.write_template(
            "site",
            "macros/note.html",
            "{% macro show() %}SITE{% endmacro show %}",
        );
        let mut env = build_markup_env(&o.cfg(), empty_meta_snapshot()).unwrap();
        // One import for the single stem, even though both layers define it.
        assert_eq!(
            env.macro_preamble,
            "{% import \"macros/note.html\" as note %}\n"
        );
        let body = format!("{}{{{{ note::show() }}}}", env.macro_preamble);
        let out = env.tera.render_str(&body, &tera::Context::new()).unwrap();
        assert_eq!(out, "SITE");
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

    // Tera has no inline map-literal syntax, so entries tests seed the map
    // through the render context.
    fn ctx_with_map() -> tera::Context {
        let mut ctx = tera::Context::new();
        let map: HashMap<&str, i32> = [("b", 2), ("a", 1), ("c", 3)].into_iter().collect();
        ctx.insert("m", &map);
        ctx
    }

    #[test]
    fn entries_sorts_map_by_key_ascending_by_default() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let out = env
            .render_str(
                "{% for e in m | entries %}{{ e.key }}={{ e.value }} {% endfor %}",
                &ctx_with_map(),
            )
            .unwrap();
        assert_eq!(out, "a=1 b=2 c=3 ");
    }

    #[test]
    fn entries_desc_reverses_key_order() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let out = env
            .render_str(
                "{% for e in m | entries(sort=\"desc\") %}{{ e.key }} {% endfor %}",
                &ctx_with_map(),
            )
            .unwrap();
        assert_eq!(out, "c b a ");
    }

    #[test]
    fn entries_is_registered_on_markup_env() {
        // Both envs get `entries` — it is pure data shaping, like the text filters.
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let out = env
            .tera
            .render_str(
                "{% for e in m | entries %}{{ e.key }} {% endfor %}",
                &ctx_with_map(),
            )
            .unwrap();
        assert_eq!(out, "a b c ");
    }

    #[test]
    fn entries_rejects_non_map_input() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        assert!(
            env.render_str("{{ [1, 2, 3] | entries }}", &tera::Context::new())
                .is_err()
        );
    }

    #[test]
    fn entries_rejects_unknown_sort_direction() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        assert!(
            env.render_str("{{ m | entries(sort=\"sideways\") }}", &ctx_with_map())
                .is_err()
        );
    }

    #[test]
    fn markdown_block_filter_on_markup_env() {
        let mut env = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        let out = env
            .tera
            .render_str(
                "{% filter markdown %}# Hi{% endfilter %}",
                &tera::Context::new(),
            )
            .unwrap();
        assert!(out.contains("<h1>"), "expected an <h1>, got: {out}");
    }

    #[test]
    fn markdown_pipe_filter_on_template_env() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let out = env
            .render_str("{{ \"*hi*\" | markdown }}", &tera::Context::new())
            .unwrap();
        assert!(out.contains("<em>hi</em>"), "expected <em>hi</em>, got: {out}");
    }

    #[test]
    fn markdown_filter_registered_on_both_envs() {
        let mut markup = build_markup_env(&cfg_without_templates(), empty_meta_snapshot()).unwrap();
        assert!(
            markup
                .tera
                .render_str("{{ \"*x*\" | markdown }}", &tera::Context::new())
                .is_ok()
        );
        let mut template = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        assert!(
            template
                .render_str("{{ \"*x*\" | markdown }}", &tera::Context::new())
                .is_ok()
        );
    }

    #[test]
    fn markdown_filter_rejects_non_string_input() {
        let mut env = build_template_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        assert!(
            env.render_str("{{ 5 | markdown }}", &tera::Context::new())
                .is_err()
        );
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
