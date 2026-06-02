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
        "templates/home.html",
        include_str!("../../scaffold/templates/home.html"),
    ),
    (
        "templates/post.html",
        include_str!("../../scaffold/templates/post.html"),
    ),
    (
        "templates/archive.html",
        include_str!("../../scaffold/templates/archive.html"),
    ),
    (
        "templates/tag.html",
        include_str!("../../scaffold/templates/tag.html"),
    ),
    (
        "templates/sitemap.xml",
        include_str!("../../scaffold/templates/sitemap.xml"),
    ),
    (
        "content/sitemap.html",
        include_str!("../../scaffold/content/sitemap.html"),
    ),
    (
        "archives/rss.xml",
        include_str!("../../scaffold/archives/rss.xml"),
    ),
    (
        "archives/posts.html",
        include_str!("../../scaffold/archives/posts.html"),
    ),
    (
        "archives/tags.html",
        include_str!("../../scaffold/archives/tags.html"),
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
    use crate::test_util::temp_path;

    #[test]
    fn errors_when_target_exists() {
        let dir = temp_path("exists");
        fs::create_dir_all(&dir).unwrap();
        let err = run(&dir).unwrap_err();
        assert!(err.to_string().contains("already exists"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn writes_all_scaffold_files() {
        let dir = temp_path("writes");
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
        let dir = temp_path("nested");
        run(&dir).unwrap();
        assert!(dir.join("content/posts/hello.md").exists());
        assert!(dir.join("templates/base.html").exists());
        assert!(dir.join("archives/rss.xml").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
