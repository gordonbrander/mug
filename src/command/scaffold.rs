use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

/// Files written by `mug new <dir>`. Sourced from `scaffold/` at the crate
/// root and embedded at compile time. Add a row here to ship a new file.
const SCAFFOLD_FILES: &[(&str, &str)] = &[
    ("config.yaml", include_str!("../../scaffold/config.yaml")),
    ("content/index.md", include_str!("../../scaffold/content/index.md")),
    (
        "content/posts/hello.md",
        include_str!("../../scaffold/content/posts/hello.md"),
    ),
    (
        "templates/base.html",
        include_str!("../../scaffold/templates/base.html"),
    ),
    (
        "generators/rss.xml",
        include_str!("../../scaffold/generators/rss.xml"),
    ),
    (
        "generators/sitemap.xml",
        include_str!("../../scaffold/generators/sitemap.xml"),
    ),
    ("static/.gitkeep", include_str!("../../scaffold/static/.gitkeep")),
];

/// Scaffold a starter site into `target`. Errors if `target` already exists at
/// all (even an empty dir): there is no merge or overwrite path in v1.
pub fn run(target: &Path) -> Result<()> {
    if target.exists() {
        bail!("path already exists: {}", target.display());
    }
    fs::create_dir_all(target)
        .with_context(|| format!("creating {}", target.display()))?;

    for (rel, contents) in SCAFFOLD_FILES {
        let dest = target.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(&dest, contents)
            .with_context(|| format!("writing {}", dest.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tempdir(name: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "mug-scaffold-{}-{}-{}",
            name,
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn errors_when_target_exists() {
        let dir = tempdir("exists");
        fs::create_dir_all(&dir).unwrap();
        let err = run(&dir).unwrap_err();
        assert!(err.to_string().contains("already exists"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn writes_all_scaffold_files() {
        let dir = tempdir("writes");
        run(&dir).unwrap();
        for (rel, _) in SCAFFOLD_FILES {
            let path = dir.join(rel);
            assert!(path.exists(), "missing {}", path.display());
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn creates_nested_parent_dirs() {
        // content/posts/hello.md requires nested-dir creation.
        let dir = tempdir("nested");
        run(&dir).unwrap();
        assert!(dir.join("content/posts/hello.md").exists());
        assert!(dir.join("templates/base.html").exists());
        assert!(dir.join("generators/sitemap.xml").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
