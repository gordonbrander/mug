//! Recursive file-tree copy, shared by the build's static-asset overlay and the
//! `scaffold` command's content copy.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Tally of what [`copy_tree`] did, so callers can report it.
pub(crate) struct CopyReport {
    pub written: usize,
    pub skipped: usize,
}

/// Copy every file under `root` into `dest`, preserving subpaths. A missing
/// `root` is a no-op (returns an all-zero report). When `overwrite` is false,
/// files that already exist at `dest` are left untouched and counted as skipped.
pub(crate) fn copy_tree(root: &Path, dest: &Path, overwrite: bool) -> Result<CopyReport> {
    let mut report = CopyReport {
        written: 0,
        skipped: 0,
    };
    if !root.exists() {
        return Ok(report);
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
        let target = dest.join(rel);
        if !overwrite && target.exists() {
            report.skipped += 1;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::copy(entry.path(), &target).with_context(|| {
            format!("copying {} -> {}", entry.path().display(), target.display())
        })?;
        report.written += 1;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cleanup, tempdir};

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    #[test]
    fn overwrite_replaces_existing() {
        let base = tempdir("copy");
        let src = base.join("src");
        let dest = base.join("dest");
        write(&src.join("a.txt"), "new");
        write(&dest.join("a.txt"), "old");
        let report = copy_tree(&src, &dest, true).unwrap();
        assert_eq!(report.written, 1);
        assert_eq!(report.skipped, 0);
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "new");
        cleanup(&base);
    }

    #[test]
    fn no_overwrite_skips_existing() {
        let base = tempdir("copy");
        let src = base.join("src");
        let dest = base.join("dest");
        write(&src.join("a.txt"), "new");
        write(&src.join("nested/b.txt"), "fresh");
        write(&dest.join("a.txt"), "old");
        let report = copy_tree(&src, &dest, false).unwrap();
        // a.txt is preserved; nested/b.txt is newly written.
        assert_eq!(report.written, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "old");
        assert_eq!(
            fs::read_to_string(dest.join("nested/b.txt")).unwrap(),
            "fresh"
        );
        cleanup(&base);
    }

    #[test]
    fn missing_root_is_noop() {
        let base = tempdir("copy");
        let report = copy_tree(&base.join("nope"), &base.join("dest"), false).unwrap();
        assert_eq!(report.written, 0);
        assert_eq!(report.skipped, 0);
        cleanup(&base);
    }
}
