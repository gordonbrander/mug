use crate::defaults::Defaults;
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
    /// When set, the markup phase scans Markdown bodies for inline `#hashtag`s,
    /// adds them to each doc's `tags`, and strips them from the rendered HTML.
    /// Off by default so literal `#` in prose is untouched. Sourced from the
    /// top-level `hashtags:` key in `config.yaml`.
    pub hashtags: bool,
    /// Origin for absolute URLs (e.g. `https://example.com`), no trailing slash.
    /// Sourced from `site.url`. Filters that produce absolute URLs degrade
    /// gracefully (to root-relative) when `None`.
    #[serde(skip)]
    pub site_url: Option<String>,
    /// Subpath the site is hosted under (e.g. `/blog`); empty for root.
    /// Sourced from `site.base_path`. Normalized to start with `/` and not end
    /// with `/`.
    #[serde(skip)]
    pub base_path: String,
    /// Glob-scoped default frontmatter from the `defaults:` key. Merged into
    /// each authored doc's frontmatter during the read phase, before fields
    /// are uplifted and the permalink is expanded.
    #[serde(skip)]
    pub defaults: Defaults,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            content_dir: PathBuf::from("content"),
            output_dir: PathBuf::from("public"),
            templates_dir: PathBuf::from("templates"),
            static_dir: PathBuf::from("static"),
            data_dir: PathBuf::from("data"),
            generators_dir: PathBuf::from("generators"),
            hashtags: false,
            site_url: None,
            base_path: String::new(),
            defaults: Defaults::default(),
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
        let mut config: Self = serde_yaml_ng::from_str(&source)
            .with_context(|| format!("parsing {} into Config", path.display()))?;
        let raw: Value = serde_yaml_ng::from_str(&source)
            .with_context(|| format!("parsing {} as YAML", path.display()))?;
        let (site, defaults_map) = match raw {
            Value::Mapping(mut m) => {
                let site = match m.remove(Value::String("site".into())) {
                    Some(Value::Mapping(s)) => s,
                    _ => Mapping::new(),
                };
                let defaults = match m.remove(Value::String("defaults".into())) {
                    Some(Value::Mapping(d)) => Some(d),
                    _ => None,
                };
                (site, defaults)
            }
            _ => (Mapping::new(), None),
        };
        if let Some(map) = defaults_map {
            config.defaults = Defaults::from_yaml_mapping(&map)
                .with_context(|| format!("parsing `defaults` in {}", path.display()))?;
        }
        config.site_url = site
            .get(Value::String("url".into()))
            .and_then(|v| v.as_str())
            .and_then(normalize_site_url);
        config.base_path = site
            .get(Value::String("base_path".into()))
            .and_then(|v| v.as_str())
            .map(normalize_base_path)
            .unwrap_or_default();
        Ok((config, site))
    }
}

fn normalize_site_url(s: &str) -> Option<String> {
    let trimmed = s.trim_end_matches('/');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_base_path(s: &str) -> String {
    let trimmed = s.trim_matches('/');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("/{}", trimmed)
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
        assert_eq!(config.output_dir, PathBuf::from("public"));
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

    #[test]
    fn normalize_site_url_strips_trailing_slash() {
        assert_eq!(
            normalize_site_url("https://example.com/"),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            normalize_site_url("https://example.com"),
            Some("https://example.com".to_string())
        );
        assert_eq!(normalize_site_url(""), None);
        assert_eq!(normalize_site_url("/"), None);
    }

    #[test]
    fn normalize_base_path_re_anchors() {
        assert_eq!(normalize_base_path(""), "");
        assert_eq!(normalize_base_path("/"), "");
        assert_eq!(normalize_base_path("blog"), "/blog");
        assert_eq!(normalize_base_path("/blog"), "/blog");
        assert_eq!(normalize_base_path("/blog/"), "/blog");
        assert_eq!(normalize_base_path("blog/sub"), "/blog/sub");
    }

    #[test]
    fn site_url_and_base_path_loaded_from_site_submap() {
        let dir = tempdir();
        let path = write_config(
            &dir,
            "site:\n  url: https://example.com/\n  base_path: blog\n",
        );
        let (config, site) = Config::load(&path).unwrap();
        assert_eq!(config.site_url.as_deref(), Some("https://example.com"));
        assert_eq!(config.base_path, "/blog");
        // Raw site map still surfaces the original values for templates.
        assert_eq!(
            site.get(Value::String("url".into()))
                .and_then(|v| v.as_str()),
            Some("https://example.com/")
        );
        cleanup(&dir);
    }

    #[test]
    fn site_url_and_base_path_default_when_absent() {
        let dir = tempdir();
        let path = write_config(&dir, "site:\n  title: My Site\n");
        let (config, _) = Config::load(&path).unwrap();
        assert!(config.site_url.is_none());
        assert_eq!(config.base_path, "");
        cleanup(&dir);
    }

    #[test]
    fn defaults_block_is_parsed() {
        let dir = tempdir();
        let path = write_config(
            &dir,
            "defaults:\n  \"posts/*.md\":\n    permalink: \":yyyy/:mm/:dd/:slug\"\n",
        );
        let (config, _) = Config::load(&path).unwrap();
        let mut data = Mapping::new();
        // A matching glob fills the default permalink.
        assert!(config.defaults.apply(Path::new("posts/hello.md"), &mut data));
        assert_eq!(
            data.get(Value::String("permalink".into()))
                .and_then(|v| v.as_str()),
            Some(":yyyy/:mm/:dd/:slug")
        );
        cleanup(&dir);
    }

    #[test]
    fn defaults_absent_yields_empty() {
        let dir = tempdir();
        let path = write_config(&dir, "content_dir: foo\n");
        let (config, _) = Config::load(&path).unwrap();
        let mut data = Mapping::new();
        assert!(!config.defaults.apply(Path::new("posts/hello.md"), &mut data));
        cleanup(&dir);
    }

    #[test]
    fn no_config_file_yields_empty_url_fields() {
        let path = Path::new("/definitely/does/not/exist/config.yaml");
        let (config, _) = Config::load(path).unwrap();
        assert!(config.site_url.is_none());
        assert_eq!(config.base_path, "");
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("mug-config-{}-{}", std::process::id(), rand_u32()));
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
