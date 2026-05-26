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

    let temp_root = std::env::temp_dir().join(format!("knead-test-{}", fixture));
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
    let build_result = knead::build();
    std::env::set_current_dir(&prev_cwd).unwrap();

    build_result.unwrap();

    let actual = collect_files(&temp_root.join("dist"));
    let expected = collect_files(&expected_dir);

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
            msg.push_str(&format!("  expected: {} ({} bytes)\n", p.display(), b.len()));
        }
        for (p, b) in &actual {
            msg.push_str(&format!("  actual:   {} ({} bytes)\n", p.display(), b.len()));
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
        let _ = fs::remove_dir_all(&temp_root);
        panic!("{}", msg);
    }

    let _ = fs::remove_dir_all(&temp_root);
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
