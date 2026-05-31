use crate::frontmatter;
use crate::permalink;
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize)]
pub struct Doc {
    pub id_path: PathBuf,
    pub output_path: PathBuf,
    pub template: Option<String>,
    pub title: String,
    pub summary: String,
    pub content: String,
    /// Term memberships, keyed by taxonomy name → (slug → display text). Built
    /// from each taxonomy's frontmatter field (e.g. `tags:`, `categories:`) and,
    /// for the built-in `tags` taxonomy when the `hashtags` config flag is set,
    /// from inline `#hashtag`s found in the body during the markup phase. The
    /// inner `BTreeMap` keeps iteration deterministic (sorted by slug) and
    /// collapses terms that slugify identically. Serializes to a nested object
    /// for templates: `{% for slug, text in page.terms.tags %}`.
    pub terms: BTreeMap<String, BTreeMap<String, String>>,
    pub date: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub data: Mapping,
    pub links: Vec<PathBuf>,
}

/// Read-only projection of `Doc` used as the frozen index view during the
/// markup phase. Omits `content` (mutated by `markup::render`), `links`
/// (populated by `markup::render`), `data` (raw frontmatter — uplifted fields
/// cover the typical uses), and `template` (a per-doc instruction, not
/// cross-doc metadata). The type system then enforces that wikilink
/// resolution and URL filters cannot read another doc's body or stale
/// markup-phase state.
#[derive(Clone, Serialize)]
pub struct DocMeta {
    pub id_path: PathBuf,
    pub output_path: PathBuf,
    pub title: String,
    pub summary: String,
    pub terms: BTreeMap<String, BTreeMap<String, String>>,
    pub date: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

impl From<&Doc> for DocMeta {
    fn from(doc: &Doc) -> DocMeta {
        DocMeta {
            id_path: doc.id_path.clone(),
            output_path: doc.output_path.clone(),
            title: doc.title.clone(),
            summary: doc.summary.clone(),
            terms: doc.terms.clone(),
            date: doc.date,
            updated: doc.updated,
        }
    }
}

/// Derived from `id_path` extension at dispatch time — there is no `kind`
/// field on `Doc` to keep in sync. `read::run` filters authored content to
/// `md|html|yaml`. The default arm is `Raw` so generator-emitted docs
/// (`.xml`, etc.) and authored `.html` pass through Tera without running
/// through comrak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Markdown,
    Raw,
    Yaml,
}

impl From<&Path> for DocKind {
    fn from(value: &Path) -> Self {
        match value.extension().and_then(|e| e.to_str()) {
            Some("md") => DocKind::Markdown,
            Some("yaml") => DocKind::Yaml,
            Some("yml") => DocKind::Yaml,
            _ => DocKind::Raw,
        }
    }
}

impl Default for Doc {
    fn default() -> Doc {
        Doc {
            id_path: PathBuf::new(),
            output_path: PathBuf::new(),
            template: None,
            title: String::new(),
            summary: String::new(),
            content: String::new(),
            terms: BTreeMap::new(),
            date: DateTime::<Utc>::UNIX_EPOCH,
            updated: DateTime::<Utc>::UNIX_EPOCH,
            data: Mapping::new(),
            links: Vec::new(),
        }
    }
}

impl Doc {
    /// Assemble a Doc from already-split inputs, holding the raw frontmatter in
    /// `data` and the unrendered body in `content`. Derived fields (`title`,
    /// `terms`, `date`, …) stay at their defaults until [`Doc::uplift_frontmatter`] runs;
    /// `output_path` starts as the path-mirroring default so an un-uplifted Doc
    /// still has a sane location.
    pub fn new(id_path: PathBuf, content: String, data: Mapping) -> Doc {
        let output_path = permalink::default_for(&id_path);
        Doc {
            id_path,
            output_path,
            content,
            data,
            ..Doc::default()
        }
    }

    /// Derive the metadata fields from `self.data` (spec §5): `title`, `summary`,
    /// `template`, `terms` (one bucket per taxonomy, read from its frontmatter
    /// field), and `output_path` (expanding any `permalink`). `date`/`updated`
    /// are overwritten **only** when frontmatter supplies a parseable value, so
    /// any pre-seeded value (the filesystem dates set by [`Doc::load`], or the
    /// result of an earlier uplift before [`Doc::apply_defaults`]) is preserved
    /// otherwise.
    pub fn uplift_frontmatter(&mut self, taxonomies: &[String]) {
        self.title = uplift_string(&self.data, "title").unwrap_or_default();
        self.summary = uplift_string(&self.data, "summary").unwrap_or_default();
        self.template = uplift_string(&self.data, "template");
        self.terms = uplift_terms(&self.data, taxonomies);
        if let Some(date) = parse_date(self.data.get("date")) {
            self.date = date;
        }
        if let Some(updated) = parse_date(self.data.get("updated")) {
            self.updated = updated;
        }
        self.output_path = resolve_output_path(&self.id_path, &self.data, &self.date);
    }

    /// Split frontmatter out of `source` and build a Doc (un-uplifted; call
    /// [`Doc::uplift_frontmatter`] to derive fields). Returns Err only if a present
    /// frontmatter block contains malformed YAML; missing or unterminated
    /// frontmatter is silently treated as no frontmatter (see `frontmatter::split`).
    pub fn parse(id_path: PathBuf, source: &str) -> Result<Doc> {
        let (data, body) = frontmatter::parse(source)?;
        Ok(Doc::new(id_path, body, data))
    }

    /// Build a Doc from a `.yaml` source: the whole file is the data map; the
    /// `content` field (if present and a string) becomes the body. Non-string
    /// or missing `content` defaults to an empty body. Un-uplifted; call
    /// [`Doc::uplift_frontmatter`] to derive fields.
    pub fn parse_yaml(id_path: PathBuf, source: &str) -> Result<Doc> {
        let (data, content) = frontmatter::parse_yaml(source)?;
        Ok(Doc::new(id_path, content, data))
    }

    /// Returns the kind of this doc as derived from `id_path`'s extension.
    /// `.md` → Markdown; `.yaml` → Yaml; everything else → Raw. Authored
    /// content always has one of those three extensions (read::run filters);
    /// generator-emitted docs can have anything (e.g. `.xml`) and need the
    /// Raw (Tera-only, no Markdown render) treatment.
    pub fn kind(&self) -> DocKind {
        DocKind::from(self.id_path.as_path())
    }

    /// Read `content_dir.join(id_path)` from disk, parse it, seed `date`/`updated`
    /// from filesystem metadata, then uplift frontmatter — which overrides those
    /// dates when the frontmatter supplies parseable values. So frontmatter wins
    /// and the filesystem fills in (`date`: created → modified; `updated`: modified).
    pub fn load(content_dir: &Path, id_path: &Path, taxonomies: &[String]) -> Result<Doc> {
        let fs_path = content_dir.join(id_path);
        let source = fs::read_to_string(&fs_path)
            .with_context(|| format!("could not read {}", fs_path.display()))?;
        let meta = fs::metadata(&fs_path)
            .with_context(|| format!("could not stat {}", fs_path.display()))?;
        let (data, body) = match DocKind::from(id_path) {
            DocKind::Yaml => frontmatter::parse_yaml(&source),
            _ => frontmatter::parse(&source),
        }
        .with_context(|| format!("could not parse {}", fs_path.display()))?;
        let mut doc = Doc::new(id_path.to_path_buf(), body, data);
        // Seed dates from the filesystem (created → modified for `date`, modified
        // for `updated`). `uplift_frontmatter` then overwrites either one when
        // frontmatter supplies a parseable value, and computes `output_path`
        // against the resulting date — so frontmatter wins, filesystem fills in.
        doc.date = meta
            .created()
            .ok()
            .map(DateTime::<Utc>::from)
            .or_else(|| meta.modified().ok().map(DateTime::<Utc>::from))
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
        doc.updated = meta
            .modified()
            .ok()
            .map(DateTime::<Utc>::from)
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
        doc.uplift_frontmatter(taxonomies);
        Ok(doc)
    }

    /// Merge per-collection `defaults` into this doc's frontmatter, filling only
    /// keys the doc didn't set itself (the doc's own frontmatter always wins).
    /// This only touches `self.data`; the caller re-derives fields by calling
    /// [`Doc::uplift_frontmatter`] afterwards (see `build::defaults`).
    pub fn apply_defaults(&mut self, defaults: &Mapping) {
        for (k, v) in defaults {
            if !self.data.contains_key(k) {
                self.data.insert(k.clone(), v.clone());
            }
        }
    }
}

fn resolve_output_path(id_path: &Path, data: &Mapping, date: &DateTime<Utc>) -> PathBuf {
    match data.get("permalink").and_then(Value::as_str) {
        Some(pattern) => permalink::expand(pattern, id_path, date, None),
        None => permalink::default_for(id_path),
    }
}

fn uplift_string(data: &Mapping, key: &str) -> Option<String> {
    data.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Build the per-doc `terms` map: one bucket per configured taxonomy, read from
/// that taxonomy's frontmatter `field` as a string sequence. A bucket is always
/// created for every taxonomy (even when empty) so the shape is stable for
/// templates and the markup-phase hashtag pass can find `terms["tags"]`.
fn uplift_terms(
    data: &Mapping,
    taxonomies: &[String],
) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut terms: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for name in taxonomies {
        let bucket = terms.entry(name.clone()).or_default();
        if let Some(Value::Sequence(seq)) = data.get(name.as_str()) {
            for v in seq {
                if let Some(text) = v.as_str() {
                    insert_term(bucket, text);
                }
            }
        }
    }
    terms
}

/// Slugify `text` and insert it under that slug → display text, first-write-
/// wins (so a frontmatter term's display text survives a later slug-colliding
/// inline hashtag). No-op for text that slugifies to empty. Shared by
/// `uplift_terms` and the markup-phase hashtag pass.
pub(crate) fn insert_term(bucket: &mut BTreeMap<String, String>, text: &str) {
    let slug = slug::slugify(text);
    if slug.is_empty() {
        return;
    }
    bucket.entry(slug).or_insert_with(|| text.to_string());
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

    /// The default taxonomy set used in most tests: just built-in `tags`.
    fn tax() -> Vec<String> {
        vec!["tags".to_string()]
    }

    /// Construct a Doc and uplift its metadata — the read-phase pairing.
    fn uplifted(id_path: &str, content: &str, data: Mapping, taxonomies: &[String]) -> Doc {
        let mut d = Doc::new(PathBuf::from(id_path), content.to_string(), data);
        d.uplift_frontmatter(taxonomies);
        d
    }

    #[test]
    fn default_doc_uses_unix_epoch() {
        let d = Doc::default();
        assert_eq!(d.date, DateTime::<Utc>::UNIX_EPOCH);
        assert_eq!(d.updated, DateTime::<Utc>::UNIX_EPOCH);
        assert_eq!(d.title, "");
        assert_eq!(d.summary, "");
        assert!(d.terms.is_empty());
        assert!(d.template.is_none());
    }

    #[test]
    fn uplift_frontmatter_summary_from_frontmatter() {
        let data = map_from(&[("summary", Value::String("Hand-written blurb.".into()))]);
        let d = uplifted("p.md", "", data, &tax());
        assert_eq!(d.summary, "Hand-written blurb.");
    }

    #[test]
    fn uplift_frontmatter_defaults_summary_to_empty_when_missing() {
        let d = uplifted("p.md", "", Mapping::new(), &tax());
        assert_eq!(d.summary, "");
    }

    #[test]
    fn uplift_frontmatter_title_and_template() {
        let data = map_from(&[
            ("title", Value::String("Hi".into())),
            ("template", Value::String("base.html".into())),
        ]);
        let d = uplifted("post.md", "", data, &tax());
        assert_eq!(d.title, "Hi");
        assert_eq!(d.template.as_deref(), Some("base.html"));
    }

    #[test]
    fn uplift_frontmatter_tags_as_string_sequence() {
        let data = map_from(&[(
            "tags",
            Value::Sequence(vec![
                Value::String("rust".into()),
                Value::String("ssg".into()),
                Value::Number(serde_yaml_ng::Number::from(7)),
            ]),
        )]);
        let d = uplifted("p.md", "", data, &tax());
        // Keyed by slug → display text; non-string entries (the `7`) are skipped.
        let tags = &d.terms["tags"];
        assert_eq!(tags.len(), 2);
        assert_eq!(tags["rust"], "rust");
        assert_eq!(tags["ssg"], "ssg");
    }

    #[test]
    fn uplift_frontmatter_custom_taxonomy_field() {
        let data = map_from(&[(
            "categories",
            Value::Sequence(vec![Value::String("Tech".into())]),
        )]);
        let taxonomies = vec!["tags".to_string(), "categories".to_string()];
        let d = uplifted("p.md", "", data, &taxonomies);
        assert_eq!(d.terms["categories"]["tech"], "Tech");
        // `tags` bucket exists but is empty.
        assert!(d.terms["tags"].is_empty());
    }

    #[test]
    fn uplift_frontmatter_defaults_when_keys_missing() {
        let d = uplifted("p.md", "", Mapping::new(), &tax());
        assert_eq!(d.title, "");
        // A bucket is created per taxonomy even with no terms.
        assert!(d.terms["tags"].is_empty());
        assert!(d.template.is_none());
        assert_eq!(d.date, DateTime::<Utc>::UNIX_EPOCH);
    }

    #[test]
    fn new_preserves_keys_in_data() {
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
        let d = Doc::load(&dir, Path::new("post.md"), &tax()).unwrap();
        assert_eq!(d.date.to_rfc3339(), "2025-10-31T00:00:00+00:00");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn kind_dispatches_on_extension() {
        let md = Doc::new(PathBuf::from("p.md"), String::new(), Mapping::new());
        let html = Doc::new(PathBuf::from("p.html"), String::new(), Mapping::new());
        let yaml = Doc::new(PathBuf::from("p.yaml"), String::new(), Mapping::new());
        let xml = Doc::new(PathBuf::from("p.xml"), String::new(), Mapping::new());
        let extless = Doc::new(PathBuf::from("p"), String::new(), Mapping::new());
        assert_eq!(md.kind(), DocKind::Markdown);
        assert_eq!(html.kind(), DocKind::Raw);
        assert_eq!(yaml.kind(), DocKind::Yaml);
        // Unknown / extensionless default to Raw so generator-emitted docs
        // (e.g. sitemap.xml) bypass the Markdown render.
        assert_eq!(xml.kind(), DocKind::Raw);
        assert_eq!(extless.kind(), DocKind::Raw);
    }

    #[test]
    fn parse_yaml_extracts_content_and_uplifts() {
        let source = "title: Hi\ntags: [a, b]\ndate: 2025-10-31\ncontent: \"<p>body</p>\"\n";
        let mut d = Doc::parse_yaml(PathBuf::from("p.yaml"), source).unwrap();
        d.uplift_frontmatter(&tax());
        assert_eq!(d.title, "Hi");
        assert_eq!(d.terms["tags"]["a"], "a");
        assert_eq!(d.terms["tags"]["b"], "b");
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
        let d = Doc::load(&dir, Path::new("doc.yaml"), &tax()).unwrap();
        assert_eq!(d.title, "From YAML");
        assert_eq!(d.content, "<p>hi</p>");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_falls_back_to_fs_metadata_when_no_date() {
        let dir = tempdir();
        let path = dir.join("post.md");
        fs::write(&path, "# Hello\n").unwrap();
        let d = Doc::load(&dir, Path::new("post.md"), &tax()).unwrap();
        // No frontmatter date — should pick up something more recent than the epoch
        // from the just-created file's metadata.
        assert!(d.date > DateTime::<Utc>::UNIX_EPOCH);
        assert!(d.updated > DateTime::<Utc>::UNIX_EPOCH);
        let _ = fs::remove_dir_all(&dir);
    }

    fn mapping(s: &str) -> Mapping {
        serde_yaml_ng::from_str(s).unwrap()
    }

    #[test]
    fn apply_defaults_fills_missing_and_recomputes_output_path() {
        // Mirror the pipeline: load (new + uplift), then defaults (apply + re-uplift).
        let mut d = uplifted("posts/hello.md", "", mapping("date: 2025-03-07\n"), &tax());
        d.apply_defaults(&mapping("permalink: \":yyyy/:mm/:dd/:slug/\"\n"));
        d.uplift_frontmatter(&tax());
        assert_eq!(d.output_path, PathBuf::from("2025/03/07/hello/index.html"));
    }

    #[test]
    fn apply_defaults_own_frontmatter_wins() {
        let mut d = uplifted("posts/hello.md", "", mapping("permalink: /custom/\n"), &tax());
        d.apply_defaults(&mapping("permalink: \":slug/\"\n"));
        d.uplift_frontmatter(&tax());
        assert_eq!(d.output_path, PathBuf::from("custom/index.html"));
    }

    #[test]
    fn apply_defaults_picks_up_template_and_taxonomy_field() {
        let taxonomies = vec!["tags".to_string(), "categories".to_string()];
        let mut d = uplifted("posts/hello.md", "", Mapping::new(), &taxonomies);
        d.apply_defaults(&mapping("template: post.html\ncategories: [Tech]\n"));
        d.uplift_frontmatter(&taxonomies);
        assert_eq!(d.template.as_deref(), Some("post.html"));
        assert_eq!(d.terms["categories"]["tech"], "Tech");
    }

    fn tempdir() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "mug-doc-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
}
