use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static CHDIR_LOCK: Mutex<()> = Mutex::new(());

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn collect_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_path_buf();
                let bytes = fs::read(&path).unwrap();
                out.push((rel, bytes));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn run_build(fixture: &str) {
    let _guard = CHDIR_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest_dir.join("tests").join("fixtures").join(fixture);
    let expected_dir = fixture_dir.join("expected");

    let temp_root = std::env::temp_dir().join(format!("italic-test-{}", fixture));
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&temp_root).unwrap();

    for entry in fs::read_dir(&fixture_dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "expected" {
            continue;
        }
        let dest = temp_root.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&entry.path(), &dest).unwrap();
        } else {
            fs::copy(entry.path(), &dest).unwrap();
        }
    }

    let prev_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_root).unwrap();
    let build_result = italic::build(false);
    std::env::set_current_dir(&prev_cwd).unwrap();

    build_result.unwrap();

    let cleanup_on_panic = AssertCtx {
        actual_root: temp_root.join("public"),
        expected_root: expected_dir,
        cleanup_root: Some(temp_root.clone()),
    };
    cleanup_on_panic.assert_match();
}

struct AssertCtx {
    actual_root: PathBuf,
    expected_root: PathBuf,
    cleanup_root: Option<PathBuf>,
}

impl AssertCtx {
    fn assert_match(self) {
        let actual = collect_files(&self.actual_root);
        let expected = collect_files(&self.expected_root);

        let matches = actual.len() == expected.len()
            && actual
                .iter()
                .zip(expected.iter())
                .all(|(a, e)| a.0 == e.0 && a.1 == e.1);

        if !matches {
            let mut msg = String::new();
            msg.push_str(&format!(
                "expected {} files, got {}\n",
                expected.len(),
                actual.len()
            ));
            for (p, b) in &expected {
                msg.push_str(&format!(
                    "  expected: {} ({} bytes)\n",
                    p.display(),
                    b.len()
                ));
            }
            for (p, b) in &actual {
                msg.push_str(&format!(
                    "  actual:   {} ({} bytes)\n",
                    p.display(),
                    b.len()
                ));
            }
            for ((ep, eb), (ap, ab)) in expected.iter().zip(actual.iter()) {
                if ep == ap && eb != ab {
                    msg.push_str(&format!(
                        "diff in {}:\n  expected: {:?}\n  actual:   {:?}\n",
                        ep.display(),
                        String::from_utf8_lossy(eb),
                        String::from_utf8_lossy(ab),
                    ));
                }
            }
            if let Some(root) = &self.cleanup_root {
                let _ = fs::remove_dir_all(root);
            }
            panic!("{}", msg);
        }

        if let Some(root) = &self.cleanup_root {
            let _ = fs::remove_dir_all(root);
        }
    }
}

#[test]
fn skeleton() {
    run_build("01_skeleton");
}

#[test]
fn frontmatter() {
    run_build("02_frontmatter");
}

#[test]
fn templates() {
    run_build("03_templates");
}

#[test]
fn input_types() {
    run_build("04_input_types");
}

#[test]
fn cascade() {
    run_build("05_cascade");
}

#[test]
fn permalinks() {
    run_build("06_permalinks");
}

#[test]
fn query() {
    run_build("07_query");
}

#[test]
fn archives() {
    run_build("08_archives");
}

#[test]
fn archive_limit() {
    run_build("20_archive_limit");
}

#[test]
fn wikilinks() {
    run_build("09_wikilinks");
}

#[test]
fn backlinks() {
    run_build("10_backlinks");
}

#[test]
fn related() {
    run_build("19_related");
}

#[test]
fn macros() {
    run_build("11_macros");
}

#[test]
fn defaults() {
    run_build("13_defaults");
}

#[test]
fn query_omit() {
    run_build("14_query_omit");
}

#[test]
fn collections_taxonomies() {
    run_build("15_collections_taxonomies");
}

#[test]
fn taxonomies() {
    run_build("16_taxonomies");
}

#[test]
fn drafts() {
    run_build("17_drafts");
}

#[test]
fn collection_self_exclude() {
    run_build("18_collection_self_exclude");
}

#[test]
fn aliases() {
    run_build("21_aliases");
}

#[test]
fn alias_collision() {
    run_build("22_alias_collision");
}

#[test]
fn scaffold() {
    // The 12_scaffold fixture has no input files — just an `expected/` dir.
    // The test exercises the real scaffold code path: scaffold into a temp dir
    // via `italic::new`, then build, then diff against `expected/`.
    let _guard = CHDIR_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("12_scaffold");
    let expected_dir = fixture_dir.join("expected");

    let temp_root = std::env::temp_dir().join("italic-test-12_scaffold");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&temp_root).unwrap();

    let demo = temp_root.join("demo");
    italic::new(&demo).unwrap();

    let prev_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&demo).unwrap();
    let build_result = italic::build(false);
    std::env::set_current_dir(&prev_cwd).unwrap();
    build_result.unwrap();

    let ctx = AssertCtx {
        actual_root: demo.join("public"),
        expected_root: expected_dir,
        cleanup_root: Some(temp_root),
    };
    ctx.assert_match();
}
