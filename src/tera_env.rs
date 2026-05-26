use crate::config::Config;
use crate::doc::Doc;
use crate::query;
use anyhow::{Context, Result};
use std::sync::Arc;
use tera::Tera;

/// Tera environment used by the markup phase. Intentionally restricted: no
/// index-backed filters/functions (`query`, `backlinks`) are registered here,
/// because the index is not yet meaningful at body-render time (spec §11).
pub fn build_markup_env(config: &Config) -> Result<Tera> {
    load_templates(config)
}

/// Tera environment used by the template phase. The full filter set lands here
/// over phases 7, 9, 10. `docs` is a frozen snapshot of the index taken at the
/// start of the template phase — closures registered here borrow from it for
/// the rest of the build.
pub fn build_template_env(config: &Config, docs: Arc<Vec<Doc>>) -> Result<Tera> {
    let mut env = load_templates(config)?;
    query::register(&mut env, docs);
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

    #[test]
    fn markup_env_does_not_register_query() {
        // Guards spec §11: `query` must never be callable from the markup env.
        // The function isn't registered anywhere in Phase 3, but this test
        // becomes meaningful once Phase 7 registers it on the template env.
        let mut env = build_markup_env(&cfg_without_templates()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.render_str("{{ query() }}", &ctx).is_err());
    }

    #[test]
    fn markup_env_does_not_register_backlinks() {
        let mut env = build_markup_env(&cfg_without_templates()).unwrap();
        let ctx = tera::Context::new();
        assert!(env.render_str("{{ backlinks() }}", &ctx).is_err());
    }

    #[test]
    fn missing_templates_dir_is_not_an_error() {
        // Fixtures 01 and 02 have no templates/ dir; the env must still build.
        assert!(build_markup_env(&cfg_without_templates()).is_ok());
        assert!(build_template_env(&cfg_without_templates(), Arc::new(Vec::new())).is_ok());
    }

    #[test]
    fn template_env_registers_query() {
        let env =
            build_template_env(&cfg_without_templates(), Arc::new(Vec::new())).unwrap();
        // Asserts the function exists; an empty doc snapshot returns an empty list.
        let mut env = env;
        let out = env.render_str("{{ query() | length }}", &tera::Context::new()).unwrap();
        assert_eq!(out, "0");
    }
}
