use crate::config::Config;
use crate::copy::copy_tree;
use anyhow::Result;

/// Recursively copy every file under each of `config.static_roots()` into
/// `config.output_dir`, preserving subpaths. Roots are copied in order — the
/// theme's `static/` first (when a theme is configured), then the site's — so
/// the site overlays the theme on path collisions. A missing root is a no-op so
/// zero-config sites still build.
pub fn run(config: &Config) -> Result<()> {
    for root in config.static_roots() {
        // overwrite: later roots (the site's own static/) overlay earlier ones.
        copy_tree(&root, &config.output_dir, true)?;
    }
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
    fn site_static_overlays_theme_static() {
        let base = tempdir("static");
        let theme = base.join("theme");
        let site_static = base.join("static");
        let out = base.join("public");
        // `static_roots()` derives the theme's `static/` from the theme dir.
        write(&theme.join("static/shared.css"), "theme");
        write(&theme.join("static/only-theme.css"), "t");
        write(&site_static.join("shared.css"), "site");
        write(&site_static.join("only-site.css"), "s");
        let config = Config {
            static_dir: site_static,
            theme: Some(theme),
            output_dir: out.clone(),
            ..Config::default()
        };
        run(&config).unwrap();
        // Site wins on the shared path; theme-only and site-only both land.
        assert_eq!(fs::read_to_string(out.join("shared.css")).unwrap(), "site");
        assert_eq!(fs::read_to_string(out.join("only-theme.css")).unwrap(), "t");
        assert_eq!(fs::read_to_string(out.join("only-site.css")).unwrap(), "s");
        cleanup(&base);
    }

    #[test]
    fn missing_roots_are_noops() {
        let base = tempdir("static");
        let config = Config {
            static_dir: base.join("nope"),
            theme: Some(base.join("also-nope")),
            output_dir: base.join("public"),
            ..Config::default()
        };
        run(&config).unwrap();
        cleanup(&base);
    }
}
