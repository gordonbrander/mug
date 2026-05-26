use crate::config::Config;
use crate::doc::Doc;
use crate::permalink;
use crate::query;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tera::{Tera, Value};

/// Tera environment used by the markup phase. Intentionally restricted: no
/// index-listing filters/functions (`query`, `backlinks`) are registered here,
/// because the index is not yet meaningful at body-render time (spec §11).
/// The `permalink` filter is a 1:1 lookup, not an index listing, so it ships
/// on both envs (Phase 9).
pub fn build_markup_env(config: &Config, docs: Arc<Vec<Doc>>) -> Result<Tera> {
    let mut env = load_templates(config)?;
    register_permalink(&mut env, docs);
    Ok(env)
}

/// Tera environment used by the template phase. The full filter set lands here
/// over phases 7, 9, 10. `docs` is a frozen snapshot of the index taken at the
/// start of the template phase — closures registered here borrow from it for
/// the rest of the build.
pub fn build_template_env(config: &Config, docs: Arc<Vec<Doc>>) -> Result<Tera> {
    let mut env = load_templates(config)?;
    query::register(&mut env, docs.clone());
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
        assert!(env.render_str("{{ query() }}", &ctx).is_err());
    }

    #[test]
    fn markup_env_does_not_register_backlinks() {
        let mut env = build_markup_env(&cfg_without_templates(), empty_snapshot()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.render_str("{{ backlinks() }}", &ctx).is_err());
    }

    #[test]
    fn missing_templates_dir_is_not_an_error() {
        // Fixtures 01 and 02 have no templates/ dir; the env must still build.
        assert!(build_markup_env(&cfg_without_templates(), empty_snapshot()).is_ok());
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
        let res = env.render_str("{{ 'missing.md' | permalink }}", &tera::Context::new());
        assert!(res.is_err());
    }
}
