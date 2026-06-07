use crate::config::Config;
use crate::copy::copy_tree;
use anyhow::{Result, bail};
use std::path::Path;

/// Copy the configured theme's `content/` into the project's content dir.
///
/// Themes live outside the project (their path is the `theme:` key in
/// `config.yaml`) and supply templates, static assets, and config by overlay at
/// build time — so the only thing a project needs to *own* is content. This
/// command seeds that content from the theme's starter set. Existing files are
/// left untouched, so it is safe to re-run and never clobbers the user's work.
pub fn run() -> Result<()> {
    let (config, _) = Config::load_with_theme(Path::new("config.yaml"))?;
    scaffold(&config)
}

fn scaffold(config: &Config) -> Result<()> {
    let Some(theme) = config.theme.as_ref() else {
        bail!("no theme set in config.yaml; set `theme:` to a theme directory before scaffolding");
    };
    let theme_content = theme.join("content");
    if !theme_content.exists() {
        eprintln!("theme {} has no content/ to scaffold", theme.display());
        return Ok(());
    }
    let report = copy_tree(&theme_content, &config.content_dir, false)?;
    eprintln!(
        "scaffolded {} files into {} (skipped {} existing)",
        report.written,
        config.content_dir.display(),
        report.skipped
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cleanup, tempdir};
    use std::fs;
    use std::path::Path;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    #[test]
    fn copies_theme_content_tree() {
        let base = tempdir("scaffold");
        let theme = base.join("theme");
        write(&theme.join("content/index.md"), "home");
        write(&theme.join("content/work/a.md"), "project a");
        // A "binary" file (non-UTF-8 bytes) copies fine — it's a plain fs::copy.
        write(&theme.join("content/img.bin"), "\u{0}\u{1}\u{2}");
        let config = Config {
            content_dir: base.join("content"),
            theme: Some(theme),
            ..Config::default()
        };
        scaffold(&config).unwrap();
        assert_eq!(
            fs::read_to_string(base.join("content/index.md")).unwrap(),
            "home"
        );
        assert_eq!(
            fs::read_to_string(base.join("content/work/a.md")).unwrap(),
            "project a"
        );
        assert!(base.join("content/img.bin").exists());
        cleanup(&base);
    }

    #[test]
    fn skips_existing_content() {
        let base = tempdir("scaffold");
        let theme = base.join("theme");
        write(&theme.join("content/index.md"), "theme version");
        let content = base.join("content");
        write(&content.join("index.md"), "my version");
        let config = Config {
            content_dir: content.clone(),
            theme: Some(theme),
            ..Config::default()
        };
        scaffold(&config).unwrap();
        // The user's own file wins; re-running is idempotent.
        assert_eq!(
            fs::read_to_string(content.join("index.md")).unwrap(),
            "my version"
        );
        cleanup(&base);
    }

    #[test]
    fn errors_when_no_theme() {
        let base = tempdir("scaffold");
        let config = Config {
            content_dir: base.join("content"),
            theme: None,
            ..Config::default()
        };
        let err = scaffold(&config).unwrap_err();
        assert!(err.to_string().contains("no theme set"));
        cleanup(&base);
    }

    #[test]
    fn ok_when_theme_has_no_content() {
        let base = tempdir("scaffold");
        let theme = base.join("theme");
        fs::create_dir_all(&theme).unwrap();
        let config = Config {
            content_dir: base.join("content"),
            theme: Some(theme),
            ..Config::default()
        };
        // No content/ in the theme: a notice, not an error, and nothing created.
        scaffold(&config).unwrap();
        assert!(!base.join("content").exists());
        cleanup(&base);
    }
}
