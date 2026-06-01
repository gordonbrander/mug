use crate::config::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Recursively copy every file under each of `config.static_roots()` into
/// `config.output_dir`, preserving subpaths. Roots are copied in order — the
/// theme's `static/` first (when a theme is configured), then the site's — so
/// the site overlays the theme on path collisions. A missing root is a no-op so
/// zero-config sites still build.
pub fn run(config: &Config) -> Result<()> {
    for root in config.static_roots() {
        copy_tree(&root, &config.output_dir)?;
    }
    Ok(())
}

/// Copy every file under `root` into `output_dir`, preserving subpaths. A
/// missing `root` is a no-op.
fn copy_tree(root: &Path, output_dir: &Path) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(root) {
        let entry = entry.with_context(|| format!("walking {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .expect("walkdir entry is always under the root we passed in");
        let dest = output_dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::copy(entry.path(), &dest).with_context(|| {
            format!("copying {} -> {}", entry.path().display(), dest.display())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tempdir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("mug-static-{}-{}", std::process::id(), nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn site_static_overlays_theme_static() {
        let base = tempdir();
        let theme_static = base.join("theme/static");
        let site_static = base.join("static");
        let out = base.join("public");
        write(&theme_static.join("shared.css"), "theme");
        write(&theme_static.join("only-theme.css"), "t");
        write(&site_static.join("shared.css"), "site");
        write(&site_static.join("only-site.css"), "s");
        let config = Config {
            static_dir: site_static,
            theme_static_dir: Some(theme_static),
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
        let base = tempdir();
        let config = Config {
            static_dir: base.join("nope"),
            theme_static_dir: Some(base.join("also-nope")),
            output_dir: base.join("public"),
            ..Config::default()
        };
        run(&config).unwrap();
        cleanup(&base);
    }
}
