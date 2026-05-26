use crate::config::Config;
use anyhow::{Context, Result};
use tera::Tera;

/// Tera environment used by the markup phase. Intentionally restricted: no
/// index-backed filters/functions (`query`, `backlinks`) are registered here,
/// because the index is not yet meaningful at body-render time (spec §11).
pub fn build_markup_env(config: &Config) -> Result<Tera> {
    load_templates(config)
}

/// Tera environment used by the template phase. The full filter set lands here
/// in later phases (`query` in Phase 7, `permalink` in Phase 9, `backlinks` in
/// Phase 10). In Phase 3 the two envs are functionally identical; the split
/// exists so later filter registration only touches this builder.
pub fn build_template_env(config: &Config) -> Result<Tera> {
    load_templates(config)
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
        assert!(build_template_env(&cfg_without_templates()).is_ok());
    }
}
