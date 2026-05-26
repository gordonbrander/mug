use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub content_dir: PathBuf,
    pub output_dir: PathBuf,
    pub templates_dir: PathBuf,
    pub static_dir: PathBuf,
    pub data_dir: PathBuf,
    pub generators_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            content_dir: PathBuf::from("content"),
            output_dir: PathBuf::from("dist"),
            templates_dir: PathBuf::from("templates"),
            static_dir: PathBuf::from("static"),
            data_dir: PathBuf::from("data"),
            generators_dir: PathBuf::from("generators"),
        }
    }
}

impl Config {
    /// Load `config.yaml` if present. Returns the typed `Config` (with absent
    /// keys filled from `Default`) and the raw `site:` sub-mapping, which is
    /// surfaced to templates as `{{ site }}`. A missing file yields defaults
    /// and an empty site map so zero-config sites still build.
    pub fn load(path: &Path) -> Result<(Self, Mapping)> {
        if !path.exists() {
            return Ok((Self::default(), Mapping::new()));
        }
        let source = fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let config: Self = serde_yaml_ng::from_str(&source)
            .with_context(|| format!("parsing {} into Config", path.display()))?;
        let raw: Value = serde_yaml_ng::from_str(&source)
            .with_context(|| format!("parsing {} as YAML", path.display()))?;
        let site = match raw {
            Value::Mapping(mut m) => match m.remove(Value::String("site".into())) {
                Some(Value::Mapping(s)) => s,
                _ => Mapping::new(),
            },
            _ => Mapping::new(),
        };
        Ok((config, site))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("config.yaml");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn missing_file_yields_defaults() {
        let path = Path::new("/definitely/does/not/exist/config.yaml");
        let (config, site) = Config::load(path).unwrap();
        assert_eq!(config.content_dir, PathBuf::from("content"));
        assert_eq!(config.output_dir, PathBuf::from("dist"));
        assert!(site.is_empty());
    }

    #[test]
    fn partial_overrides_keep_defaults_for_missing_keys() {
        let dir = tempdir();
        let path = write_config(&dir, "output_dir: build\n");
        let (config, site) = Config::load(&path).unwrap();
        assert_eq!(config.output_dir, PathBuf::from("build"));
        assert_eq!(config.content_dir, PathBuf::from("content"));
        assert!(site.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn site_submap_is_extracted() {
        let dir = tempdir();
        let path = write_config(
            &dir,
            "site:\n  title: My Site\n  description: A blog\n",
        );
        let (_, site) = Config::load(&path).unwrap();
        assert_eq!(
            site.get(Value::String("title".into()))
                .and_then(|v| v.as_str()),
            Some("My Site"),
        );
        cleanup(&dir);
    }

    #[test]
    fn site_absent_yields_empty_map() {
        let dir = tempdir();
        let path = write_config(&dir, "content_dir: foo\n");
        let (_, site) = Config::load(&path).unwrap();
        assert!(site.is_empty());
        cleanup(&dir);
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("knead-config-{}-{}", std::process::id(), rand_u32()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    fn rand_u32() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    }
}
