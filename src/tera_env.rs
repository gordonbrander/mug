use crate::backlinks;
use crate::config::Config;
use crate::doc::Doc;
use crate::permalink;
use crate::query;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tera::{Tera, Value};

/// Markup-phase Tera env paired with the auto-import preamble for
/// `templates/macros/*.html` (Phase 11). Markdown bodies can invoke macros as
/// shortcodes without writing `{% import %}` boilerplate: `markup::render`
/// prepends `macro_preamble` to every body before `render_str`.
pub struct MarkupEnv {
    pub tera: Tera,
    pub macro_preamble: String,
}

/// Tera environment used by the markup phase. Intentionally restricted: no
/// index-listing filters/functions (`query`, `backlinks`) are registered here,
/// because the index is not yet meaningful at body-render time (spec §11).
/// The `permalink` filter is a 1:1 lookup, not an index listing, so it ships
/// on both envs (Phase 9).
pub fn build_markup_env(config: &Config, docs: Arc<Vec<Doc>>) -> Result<MarkupEnv> {
    let mut tera = load_templates(config)?;
    register_permalink(&mut tera, docs);
    let macro_preamble = discover_macro_imports(config);
    Ok(MarkupEnv {
        tera,
        macro_preamble,
    })
}

/// Tera environment used by the template phase. The full filter set lands here
/// over phases 7, 9, 10. `docs` is a frozen snapshot of the index taken at the
/// start of the template phase — closures registered here borrow from it for
/// the rest of the build.
pub fn build_template_env(config: &Config, docs: Arc<Vec<Doc>>) -> Result<Tera> {
    let mut env = load_templates(config)?;
    query::register(&mut env, docs.clone());
    backlinks::register(&mut env, docs.clone());
    register_permalink(&mut env, docs);
    Ok(env)
}

/// Register the `permalink` Tera filter: given a string `id_path`, return the
/// corresponding doc's `output_path` as a URL. The closure captures an
/// `Arc<Vec<Doc>>` snapshot of the index so it sees a frozen view for the rest
/// of the phase. Used by both envs (Phase 9).
fn register_permalink(env: &mut Tera, docs: Arc<Vec<Doc>>) {
    env.register_filter(
        "permalink",
        move |value: &Value, _args: &HashMap<String, Value>| -> tera::Result<Value> {
            let id_path_str = value.as_str().ok_or_else(|| {
                tera::Error::msg("permalink filter: input must be a string id_path")
            })?;
            let target = PathBuf::from(id_path_str);
            let doc = docs.iter().find(|d| d.id_path == target).ok_or_else(|| {
                tera::Error::msg(format!(
                    "permalink filter: no doc with id_path `{}`",
                    id_path_str
                ))
            })?;
            let url = permalink::to_url(&doc.output_path);
            tera::to_value(url).map_err(tera::Error::from)
        },
    );
}

fn load_templates(config: &Config) -> Result<Tera> {
    if !config.templates_dir.exists() {
        return Ok(Tera::default());
    }
    let pattern = format!("{}/**/*.{{html,xml}}", config.templates_dir.display());
    Tera::new(&pattern).with_context(|| format!("loading templates from {}", pattern))
}

/// Scan `templates/macros/*.html` (non-recursive) and build a Tera import
/// preamble: one `{% import "macros/<stem>.html" as <stem> %}` per macro file.
/// Sorted alphabetically by stem so output is deterministic across platforms
/// (the FS read order is not). Returns `""` if `templates/` or
/// `templates/macros/` is missing, or the dir contains no `.html` files.
fn discover_macro_imports(config: &Config) -> String {
    let macros_dir = config.templates_dir.join("macros");
    let Ok(entries) = fs::read_dir(&macros_dir) else {
        return String::new();
    };

    let mut stems: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("html") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            stems.push(stem.to_string());
        }
    }
    stems.sort();

    let mut out = String::new();
    for stem in stems {
        out.push_str(&format!(
            "{{% import \"macros/{stem}.html\" as {stem} %}}\n"
        ));
    }
    out
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

    fn empty_snapshot() -> Arc<Vec<Doc>> {
        Arc::new(Vec::new())
    }

    #[test]
    fn markup_env_does_not_register_query() {
        // Guards spec §11: `query` must never be callable from the markup env.
        let mut env = build_markup_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.tera.render_str("{{ query() }}", &ctx).is_err());
    }

    #[test]
    fn markup_env_does_not_register_backlinks() {
        let mut env = build_markup_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.tera.render_str("{{ backlinks() }}", &ctx).is_err());
    }

    #[test]
    fn markup_env_does_not_register_backlinks_as_filter() {
        // Spec §11: even the filter form must fail on the markup env.
        let mut env = build_markup_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.tera.render_str("{{ 'a.md' | backlinks }}", &ctx).is_err());
    }

    #[test]
    fn missing_templates_dir_is_not_an_error() {
        // Fixtures 01 and 02 have no templates/ dir; the env must still build.
        let env = build_markup_env(&cfg_without_templates(), empty_snapshot()).unwrap();
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

    #[test]
    fn markup_env_registers_permalink() {
        let mut env =
            build_markup_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
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
            build_markup_env(&cfg_without_templates(), one_doc_snapshot()).unwrap();
        let res = env
            .tera
            .render_str("{{ 'missing.md' | permalink }}", &tera::Context::new());
        assert!(res.is_err());
    }

    /// Small RAII temp-dir helper for macro-discovery tests. Removed on drop
    /// so test failures don't leak directories in /tmp.
    struct TempTemplatesDir(PathBuf);
    impl TempTemplatesDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "knead-tera_env-test-{}-{}",
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
        let mut env = build_markup_env(&tmp.cfg(), empty_snapshot()).unwrap();
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
        let env = build_markup_env(&tmp.cfg(), empty_snapshot()).unwrap();
        assert!(env.macro_preamble.is_empty());
    }

    #[test]
    fn markup_env_handles_empty_macros_dir() {
        let tmp = TempTemplatesDir::new("empty-macros");
        std::fs::create_dir_all(tmp.0.join("macros")).unwrap();
        let env = build_markup_env(&tmp.cfg(), empty_snapshot()).unwrap();
        assert!(env.macro_preamble.is_empty());
    }

    #[test]
    fn markup_env_macro_preamble_is_deterministic() {
        let tmp = TempTemplatesDir::new("sorted");
        // Write in reverse order to prove sort is by stem, not FS read order.
        tmp.write_macros_file("twitter.html", "{% macro x() %}t{% endmacro x %}");
        tmp.write_macros_file("aside.html", "{% macro x() %}a{% endmacro x %}");
        let env = build_markup_env(&tmp.cfg(), empty_snapshot()).unwrap();
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
        let env = build_markup_env(&tmp.cfg(), empty_snapshot()).unwrap();
        assert_eq!(
            env.macro_preamble,
            "{% import \"macros/real.html\" as real %}\n"
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
