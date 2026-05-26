use crate::frontmatter;
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct Doc {
    pub id_path: PathBuf,
    pub output_path: PathBuf,
    pub template: Option<String>,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub date: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub data: Mapping,
}

/// Derived from `id_path` extension at dispatch time — there is no `kind`
/// field on `Doc` to keep in sync. `read::run` already filters extensions to
/// `md|html|yaml`, so the default arm of `Doc::kind` is unreachable in
/// practice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Markdown,
    Html,
    Yaml,
}

impl Default for Doc {
    fn default() -> Doc {
        Doc {
            id_path: PathBuf::new(),
            output_path: PathBuf::new(),
            template: None,
            title: String::new(),
            content: String::new(),
            tags: Vec::new(),
            date: DateTime::<Utc>::UNIX_EPOCH,
            updated: DateTime::<Utc>::UNIX_EPOCH,
            data: Mapping::new(),
        }
    }
}

impl Doc {
    /// Assemble a Doc from already-split inputs. Uplifts `title`, `template`,
    /// `tags`, `date`, `updated` from `data` per spec §5. No filesystem
    /// fallback for dates — that's `load`'s job.
    pub fn new(id_path: PathBuf, content: String, data: Mapping) -> Doc {
        let output_path = id_path.with_extension("html");
        let title = uplift_string(&data, "title").unwrap_or_default();
        let template = uplift_string(&data, "template");
        let tags = uplift_tags(&data);
        let date = parse_date(data.get("date")).unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
        let updated = parse_date(data.get("updated")).unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
        Doc {
            id_path,
            output_path,
            template,
            title,
            content,
            tags,
            date,
            updated,
            data,
        }
    }

    /// Split frontmatter out of `source`, parse it as YAML, and build a Doc.
    /// Returns Err only if a present frontmatter block contains malformed
    /// YAML; missing or unterminated frontmatter is silently treated as no
    /// frontmatter (see `frontmatter::split`).
    pub fn parse(id_path: PathBuf, source: &str) -> Result<Doc> {
        let (data, body) = frontmatter::parse(source)?;
        Ok(Doc::new(id_path, body.to_string(), data))
    }

    /// Build a Doc from a `.yaml` source: the whole file is the data map; the
    /// `content` field (if present and a string) becomes the body. Non-string
    /// or missing `content` defaults to an empty body.
    pub fn parse_yaml(id_path: PathBuf, source: &str) -> Result<Doc> {
        let data = frontmatter::parse_yaml(source)?;
        let content = data
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default();
        Ok(Doc::new(id_path, content, data))
    }

    /// Returns the kind of this doc as derived from `id_path`'s extension.
    /// The default arm returns `Markdown` but is unreachable in practice —
    /// `read::run` filters to `md|html|yaml` before any Doc is constructed.
    pub fn kind(&self) -> DocKind {
        match self.id_path.extension().and_then(|e| e.to_str()) {
            Some("html") => DocKind::Html,
            Some("yaml") => DocKind::Yaml,
            _ => DocKind::Markdown,
        }
    }

    /// Read `content_dir.join(id_path)` from disk, parse it, and apply the
    /// filesystem-based fallback chain for `date` (created → modified) and
    /// `updated` (modified) when frontmatter didn't supply a parseable value.
    pub fn load(content_dir: &Path, id_path: &Path) -> Result<Doc> {
        let fs_path = content_dir.join(id_path);
        let source = fs::read_to_string(&fs_path)
            .with_context(|| format!("could not read {}", fs_path.display()))?;
        let meta = fs::metadata(&fs_path)
            .with_context(|| format!("could not stat {}", fs_path.display()))?;
        let parsed = match id_path.extension().and_then(|e| e.to_str()) {
            Some("yaml") => Doc::parse_yaml(id_path.to_path_buf(), &source),
            _ => Doc::parse(id_path.to_path_buf(), &source),
        };
        let mut doc =
            parsed.with_context(|| format!("could not parse {}", fs_path.display()))?;

        if parse_date(doc.data.get("date")).is_none() {
            doc.date = meta
                .created()
                .ok()
                .map(DateTime::<Utc>::from)
                .or_else(|| meta.modified().ok().map(DateTime::<Utc>::from))
                .unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
        }
        if parse_date(doc.data.get("updated")).is_none() {
            doc.updated = meta
                .modified()
                .ok()
                .map(DateTime::<Utc>::from)
                .unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
        }
        Ok(doc)
    }
}

fn uplift_string(data: &Mapping, key: &str) -> Option<String> {
    data.get(key).and_then(Value::as_str).map(str::to_string)
}

fn uplift_tags(data: &Mapping) -> Vec<String> {
    let Some(Value::Sequence(seq)) = data.get("tags") else {
        return Vec::new();
    };
    seq.iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

pub(crate) fn parse_date(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let s = value?.as_str()?;
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.and_hms_opt(0, 0, 0)?.and_utc());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml_ng::Value;
    use std::fs;
    use std::io::Write;

    fn map_from(pairs: &[(&str, Value)]) -> Mapping {
        let mut m = Mapping::new();
        for (k, v) in pairs {
            m.insert(Value::String((*k).to_string()), v.clone());
        }
        m
    }

    #[test]
    fn default_doc_uses_unix_epoch() {
        let d = Doc::default();
        assert_eq!(d.date, DateTime::<Utc>::UNIX_EPOCH);
        assert_eq!(d.updated, DateTime::<Utc>::UNIX_EPOCH);
        assert_eq!(d.title, "");
        assert!(d.tags.is_empty());
        assert!(d.template.is_none());
    }

    #[test]
    fn new_uplifts_title_and_template() {
        let data = map_from(&[
            ("title", Value::String("Hi".into())),
            ("template", Value::String("base.html".into())),
        ]);
        let d = Doc::new(PathBuf::from("post.md"), String::new(), data);
        assert_eq!(d.title, "Hi");
        assert_eq!(d.template.as_deref(), Some("base.html"));
    }

    #[test]
    fn new_uplifts_tags_as_string_sequence() {
        let data = map_from(&[(
            "tags",
            Value::Sequence(vec![
                Value::String("rust".into()),
                Value::String("ssg".into()),
                Value::Number(serde_yaml_ng::Number::from(7)),
            ]),
        )]);
        let d = Doc::new(PathBuf::from("p.md"), String::new(), data);
        assert_eq!(d.tags, vec!["rust".to_string(), "ssg".to_string()]);
    }

    #[test]
    fn new_defaults_when_keys_missing() {
        let d = Doc::new(PathBuf::from("p.md"), String::new(), Mapping::new());
        assert_eq!(d.title, "");
        assert!(d.tags.is_empty());
        assert!(d.template.is_none());
        assert_eq!(d.date, DateTime::<Utc>::UNIX_EPOCH);
    }

    #[test]
    fn new_preserves_uplifted_keys_in_data() {
        let data = map_from(&[("title", Value::String("Hi".into()))]);
        let d = Doc::new(PathBuf::from("p.md"), String::new(), data);
        assert_eq!(d.data.get("title").and_then(Value::as_str), Some("Hi"));
    }

    #[test]
    fn output_path_swaps_extension_to_html() {
        let d = Doc::new(PathBuf::from("blog/post.md"), String::new(), Mapping::new());
        assert_eq!(d.output_path, PathBuf::from("blog/post.html"));
    }

    #[test]
    fn parse_date_accepts_rfc3339() {
        let v = Value::String("2025-10-31T12:00:00Z".into());
        let dt = parse_date(Some(&v)).unwrap();
        assert_eq!(dt.to_rfc3339(), "2025-10-31T12:00:00+00:00");
    }

    #[test]
    fn parse_date_accepts_date_only() {
        let v = Value::String("2025-10-31".into());
        let dt = parse_date(Some(&v)).unwrap();
        assert_eq!(dt.to_rfc3339(), "2025-10-31T00:00:00+00:00");
    }

    #[test]
    fn parse_date_rejects_garbage() {
        let v = Value::String("not a date".into());
        assert!(parse_date(Some(&v)).is_none());
        assert!(parse_date(None).is_none());
    }

    #[test]
    fn parse_returns_err_on_malformed_yaml() {
        let source = "---\ntitle: [unterminated\n---\nbody";
        assert!(Doc::parse(PathBuf::from("p.md"), source).is_err());
    }

    #[test]
    fn parse_treats_missing_frontmatter_as_empty() {
        let d = Doc::parse(PathBuf::from("p.md"), "# Body\n").unwrap();
        assert_eq!(d.content, "# Body\n");
        assert!(d.data.is_empty());
    }

    #[test]
    fn load_uses_frontmatter_date_when_present() {
        let dir = tempdir();
        let path = dir.join("post.md");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "---\ndate: 2025-10-31\n---\nbody").unwrap();
        drop(f);
        let d = Doc::load(&dir, Path::new("post.md")).unwrap();
        assert_eq!(d.date.to_rfc3339(), "2025-10-31T00:00:00+00:00");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn kind_dispatches_on_extension() {
        let md = Doc::new(PathBuf::from("p.md"), String::new(), Mapping::new());
        let html = Doc::new(PathBuf::from("p.html"), String::new(), Mapping::new());
        let yaml = Doc::new(PathBuf::from("p.yaml"), String::new(), Mapping::new());
        let unknown = Doc::new(PathBuf::from("p.txt"), String::new(), Mapping::new());
        let extless = Doc::new(PathBuf::from("p"), String::new(), Mapping::new());
        assert_eq!(md.kind(), DocKind::Markdown);
        assert_eq!(html.kind(), DocKind::Html);
        assert_eq!(yaml.kind(), DocKind::Yaml);
        assert_eq!(unknown.kind(), DocKind::Markdown);
        assert_eq!(extless.kind(), DocKind::Markdown);
    }

    #[test]
    fn parse_yaml_extracts_content_and_uplifts() {
        let source = "title: Hi\ntags: [a, b]\ndate: 2025-10-31\ncontent: \"<p>body</p>\"\n";
        let d = Doc::parse_yaml(PathBuf::from("p.yaml"), source).unwrap();
        assert_eq!(d.title, "Hi");
        assert_eq!(d.tags, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(d.date.to_rfc3339(), "2025-10-31T00:00:00+00:00");
        assert_eq!(d.content, "<p>body</p>");
    }

    #[test]
    fn parse_yaml_defaults_content_when_missing() {
        let d = Doc::parse_yaml(PathBuf::from("p.yaml"), "title: Hi\n").unwrap();
        assert_eq!(d.content, "");
    }

    #[test]
    fn parse_yaml_defaults_content_when_non_string() {
        let d = Doc::parse_yaml(PathBuf::from("p.yaml"), "content: 42\n").unwrap();
        assert_eq!(d.content, "");
    }

    #[test]
    fn parse_yaml_errors_on_malformed_yaml() {
        assert!(Doc::parse_yaml(PathBuf::from("p.yaml"), "title: [unterminated").is_err());
    }

    #[test]
    fn parse_yaml_output_path_swaps_to_html() {
        let d = Doc::parse_yaml(PathBuf::from("page.yaml"), "title: Hi\n").unwrap();
        assert_eq!(d.output_path, PathBuf::from("page.html"));
    }

    #[test]
    fn load_dispatches_yaml_to_parse_yaml() {
        let dir = tempdir();
        let path = dir.join("doc.yaml");
        fs::write(&path, "title: From YAML\ncontent: \"<p>hi</p>\"\n").unwrap();
        let d = Doc::load(&dir, Path::new("doc.yaml")).unwrap();
        assert_eq!(d.title, "From YAML");
        assert_eq!(d.content, "<p>hi</p>");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_falls_back_to_fs_metadata_when_no_date() {
        let dir = tempdir();
        let path = dir.join("post.md");
        fs::write(&path, "# Hello\n").unwrap();
        let d = Doc::load(&dir, Path::new("post.md")).unwrap();
        // No frontmatter date — should pick up something more recent than the epoch
        // from the just-created file's metadata.
        assert!(d.date > DateTime::<Utc>::UNIX_EPOCH);
        assert!(d.updated > DateTime::<Utc>::UNIX_EPOCH);
        let _ = fs::remove_dir_all(&dir);
    }

    fn tempdir() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "knead-doc-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
}
