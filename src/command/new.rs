use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

/// Files written by `italic new <dir>`: an empty, theme-agnostic skeleton the
/// user fills in themselves. `config.yaml` is sourced from `scaffold/` at the
/// crate root and embedded at compile time; the directory markers are empty.
/// Content and themes are not shipped — set `theme:` in `config.yaml` and run
/// `italic scaffold` to pull a theme's starter content.
const SCAFFOLD_FILES: &[(&str, &str)] = &[
    ("config.yaml", include_str!("../../scaffold/config.yaml")),
    ("content/.gitkeep", ""),
    ("templates/.gitkeep", ""),
    ("data/.gitkeep", ""),
    ("static/.gitkeep", ""),
    ("archives/.gitkeep", ""),
    ("themes/.gitkeep", ""),
];

/// Scaffold an empty starter site into `target`. Errors if `target` already
/// exists at all (even an empty dir): there is no merge or overwrite path.
pub fn run(target: &Path) -> Result<()> {
    if target.exists() {
        bail!("path already exists: {}", target.display());
    }
    fs::create_dir_all(target).with_context(|| format!("creating {}", target.display()))?;

    for (rel, contents) in SCAFFOLD_FILES {
        let dest = target.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(&dest, contents).with_context(|| format!("writing {}", dest.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
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
    fn creates_skeleton_dirs() {
        // content/.gitkeep etc. require nested-dir creation.
        let dir = temp_path("skeleton");
        run(&dir).unwrap();
        assert!(dir.join("config.yaml").exists());
        assert!(dir.join("content/.gitkeep").exists());
        assert!(dir.join("templates/.gitkeep").exists());
        assert!(dir.join("data/.gitkeep").exists());
        assert!(dir.join("static/.gitkeep").exists());
        assert!(dir.join("archives/.gitkeep").exists());
        assert!(dir.join("themes/.gitkeep").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scaffolded_config_loads_as_defaults() {
        // The shipped config.yaml is all comments; loading it must succeed and
        // yield defaults (guards against a malformed example breaking fresh sites).
        let dir = temp_path("config-loads");
        run(&dir).unwrap();
        let (config, site) = Config::load_with_theme(&dir.join("config.yaml")).unwrap();
        assert_eq!(config.content_dir, std::path::PathBuf::from("content"));
        assert!(config.theme.is_none());
        assert!(site.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }
}
