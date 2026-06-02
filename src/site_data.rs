use crate::config::Config;
use anyhow::{Context, Result};
use serde_yaml_ng::{Mapping, Value};
use std::fs;

/// Site-wide context loaded once at build start. `site` is the `site:` sub-map
/// from `config.yaml`; `data` is the top-level `data/` cascade. Both are
/// injected into every Tera context as `{{ site }}` and `{{ data }}`.
pub struct SiteData {
    pub site: Mapping,
    pub data: Mapping,
}

impl SiteData {
    pub fn load(config: &Config, site: Mapping) -> Result<Self> {
        let data = load_data(config)?;
        Ok(Self { site, data })
    }
}

/// Read top-level `.yaml` / `.yml` files under `config.data_dir` into a
/// `Mapping` keyed by filename stem. Subdirectories are skipped silently in
/// v1 — top-level files only.
fn load_data(config: &Config) -> Result<Mapping> {
    let mut map = Mapping::new();
    if !config.data_dir.exists() {
        return Ok(map);
    }
    let mut entries: Vec<_> = fs::read_dir(&config.data_dir)
        .with_context(|| format!("reading {}", config.data_dir.display()))?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if !file_type.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !matches!(ext, "yaml" | "yml") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let body =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let value: Value = serde_yaml_ng::from_str(&body)
            .with_context(|| format!("parsing {}", path.display()))?;
        map.insert(Value::String(stem), value);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cleanup, tempdir};
    use std::io::Write;
    use std::path::Path;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    fn config_in(dir: &Path) -> Config {
        Config {
            data_dir: dir.join("data"),
            ..Config::default()
        }
    }

    #[test]
    fn missing_data_dir_yields_empty_map() {
        let dir = tempdir("sitedata");
        let config = config_in(&dir);
        let data = load_data(&config).unwrap();
        assert!(data.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn yaml_file_loaded_at_stem() {
        let dir = tempdir("sitedata");
        write(&dir.join("data/menu.yaml"), "items: [a, b]\n");
        let config = config_in(&dir);
        let data = load_data(&config).unwrap();
        let menu = data.get(Value::String("menu".into())).unwrap();
        let items = menu
            .as_mapping()
            .and_then(|m| m.get(Value::String("items".into())))
            .and_then(|v| v.as_sequence())
            .unwrap();
        assert_eq!(items.len(), 2);
        cleanup(&dir);
    }

    #[test]
    fn both_yaml_and_yml_extensions_load() {
        let dir = tempdir("sitedata");
        write(&dir.join("data/a.yaml"), "x: 1\n");
        write(&dir.join("data/b.yml"), "y: 2\n");
        let config = config_in(&dir);
        let data = load_data(&config).unwrap();
        assert!(data.contains_key(Value::String("a".into())));
        assert!(data.contains_key(Value::String("b".into())));
        cleanup(&dir);
    }

    #[test]
    fn non_yaml_files_are_ignored() {
        let dir = tempdir("sitedata");
        write(&dir.join("data/notes.txt"), "ignore me\n");
        write(&dir.join("data/keep.yaml"), "k: v\n");
        let config = config_in(&dir);
        let data = load_data(&config).unwrap();
        assert_eq!(data.len(), 1);
        assert!(data.contains_key(Value::String("keep".into())));
        cleanup(&dir);
    }

    #[test]
    fn subdirectories_are_skipped() {
        let dir = tempdir("sitedata");
        write(&dir.join("data/nested/inner.yaml"), "x: 1\n");
        write(&dir.join("data/top.yaml"), "y: 2\n");
        let config = config_in(&dir);
        let data = load_data(&config).unwrap();
        assert_eq!(data.len(), 1);
        assert!(data.contains_key(Value::String("top".into())));
        cleanup(&dir);
    }
}
