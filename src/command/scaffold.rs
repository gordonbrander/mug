use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

/// Files written by `mug new <dir>`. Sourced from `scaffold/` at the crate
/// root and embedded at compile time. Add a row here to ship a new file.
const SCAFFOLD_FILES: &[(&str, &str)] = &[
    ("config.yaml", include_str!("../../scaffold/config.yaml")),
    // Content: a small interlinked digital garden. Each note links to others
    // with [[wikilinks]] so the obsidian theme's backlinks have something to show.
    ("content/index.md", include_str!("../../scaffold/content/index.md")),
    (
        "content/digital-garden.md",
        include_str!("../../scaffold/content/digital-garden.md"),
    ),
    (
        "content/wikilinks.md",
        include_str!("../../scaffold/content/wikilinks.md"),
    ),
    (
        "content/backlinks.md",
        include_str!("../../scaffold/content/backlinks.md"),
    ),
    (
        "content/sitemap.html",
        include_str!("../../scaffold/content/sitemap.html"),
    ),
    ("static/.gitkeep", include_str!("../../scaffold/static/.gitkeep")),
    // The bundled "obsidian" theme: templates, styles, and config defaults for a
    // wiki / digital garden. Activated by `theme: themes/obsidian` in config.yaml.
    (
        "themes/obsidian/config.yaml",
        include_str!("../../scaffold/themes/obsidian/config.yaml"),
    ),
    (
        "themes/obsidian/templates/base.html",
        include_str!("../../scaffold/themes/obsidian/templates/base.html"),
    ),
    (
        "themes/obsidian/templates/note.html",
        include_str!("../../scaffold/themes/obsidian/templates/note.html"),
    ),
    (
        "themes/obsidian/templates/index.html",
        include_str!("../../scaffold/themes/obsidian/templates/index.html"),
    ),
    (
        "themes/obsidian/templates/sitemap.xml",
        include_str!("../../scaffold/themes/obsidian/templates/sitemap.xml"),
    ),
    (
        "themes/obsidian/static/style.css",
        include_str!("../../scaffold/themes/obsidian/static/style.css"),
    ),
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
        // themes/obsidian/templates/base.html requires nested-dir creation.
        let dir = temp_path("nested");
        run(&dir).unwrap();
        assert!(dir.join("content/index.md").exists());
        assert!(dir.join("themes/obsidian/templates/base.html").exists());
        assert!(dir.join("themes/obsidian/static/style.css").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
