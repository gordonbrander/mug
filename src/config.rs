use crate::query::Query;
use crate::related::{LINKS, Related};
use crate::taxonomy;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Name of the always-present collection of every doc. When a site/theme does
/// not declare its own `all` under `collections:`, [`Config::load_with_theme`]
/// injects one with the default [`Query`] (every doc, date desc). It backs the
/// `all()` template function (`src/tera_env/all.rs`). Mirrors the [`LINKS`]
/// reserved-name pattern.
pub const ALL: &str = "all";

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub content_dir: PathBuf,
    pub output_dir: PathBuf,
    pub templates_dir: PathBuf,
    pub static_dir: PathBuf,
    pub data_dir: PathBuf,
    pub archives_dir: PathBuf,
    /// Path to a theme folder (relative to the working directory), from the
    /// top-level `theme:` key. When set, the theme's `templates/`, `archives/`,
    /// and `static/` are overlaid *beneath* the site's: the site's own files win
    /// per-path, but anything the site doesn't provide falls through to the
    /// theme (see [`template_roots`](Self::template_roots),
    /// [`archive_roots`](Self::archive_roots), [`static_roots`](Self::static_roots)).
    /// The theme's `config.yaml` supplies config defaults the site overrides.
    /// `data`, `content`, and `output` stay site-only. A theme without a
    /// `config.yaml` still contributes files via the conventional subdir names.
    /// Themes are not nested (one level, no child themes): a `theme:` key inside
    /// a theme's own `config.yaml` is ignored.
    pub theme: Option<PathBuf>,
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
    /// Per-collection default frontmatter from the `defaults:` key. Each entry
    /// names a collection (from `collections:`); its frontmatter values fill keys
    /// the members of that collection did not set themselves. Applied after
    /// collection classification and before markup (see `build::defaults`).
    /// Config order; later entries win on overlap.
    #[serde(skip)]
    pub defaults: Vec<(String, Mapping)>,
    /// Named queries from the `collections:` key, each a [`Query`]. The classify
    /// phase evaluates them once into the `DocIndex` so templates can read them
    /// cheaply via the `collection()` function. Insertion order is the
    /// `config.yaml` order; collection lookup is by name regardless.
    #[serde(skip)]
    pub collections: Vec<(String, Query)>,
    /// Declared taxonomy names, parsed from the `taxonomies:` array in
    /// declaration order. There are no built-in defaults — a site (or the
    /// scaffold) declares `tags` like any other taxonomy. The read phase uses
    /// each name as a frontmatter field to uplift a doc's term memberships into
    /// `Doc.terms`, and the classify phase inverts those into
    /// `taxonomy → term → docs`.
    #[serde(skip)]
    pub taxonomies: Vec<String>,
    /// Weights for the `related` template filter, from the `related:` key's
    /// `weights:` sub-map (per-namespace). `limit`/`omit` are not config — they
    /// are filter arguments — so only `weights` is populated here. When the block
    /// (or its `weights`) is absent, [`load_with_theme`](Self::load_with_theme)
    /// fills in equal weight on every declared taxonomy plus the `links` graph,
    /// so the filter works zero-config (relating by `links`, and by `tags` once
    /// `tags` is declared). See [`Related`].
    #[serde(skip)]
    pub related: Related,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            content_dir: PathBuf::from("content"),
            output_dir: PathBuf::from("public"),
            templates_dir: PathBuf::from("templates"),
            static_dir: PathBuf::from("static"),
            data_dir: PathBuf::from("data"),
            archives_dir: PathBuf::from("archives"),
            theme: None,
            hashtags: false,
            site_url: None,
            base_path: String::new(),
            defaults: Vec::new(),
            collections: Vec::new(),
            taxonomies: Vec::new(),
            related: Related::default(),
        }
    }
}

impl Config {
    /// Load `config.yaml` (resolving a theme if one is set) into the typed
    /// `Config` and the raw `site:` sub-mapping surfaced to templates as
    /// `{{ site }}`. A missing file yields defaults and an empty site map so
    /// zero-config sites still build.
    ///
    /// When the file sets `theme:`, the theme's own `config.yaml` is loaded and
    /// layered underneath: it supplies templates/archives directories and config
    /// defaults (collections, taxonomies, defaults, `site:` metadata) that the
    /// site overrides. See [`apply_theme`](Self::apply_theme). The `site_url` and
    /// `base_path` fields are derived once here from the final (possibly merged)
    /// `site` map.
    pub fn load_with_theme(path: &Path) -> Result<(Self, Mapping)> {
        let (mut config, mut site) = Self::load(path)?;
        if let Some(theme_dir) = config.theme.clone() {
            let theme_config_path = theme_dir.join("config.yaml");
            let (theme_config, theme_site) = Self::load(&theme_config_path)
                .with_context(|| format!("loading theme at {}", theme_dir.display()))?;
            config.apply_theme(&mut site, theme_config, theme_site);
        }
        // Guarantee an `all` collection always exists so `all()` and sitemap/RSS
        // templates have a canonical "every doc" listing. The default Query is
        // every doc, date desc; a site or theme may declare its own `all` to
        // reorder, omit, or filter (full override, kept by `merge_named` above).
        if !config.collections.iter().any(|(n, _)| n == ALL) {
            config.collections.push((ALL.to_string(), Query::default()));
        }
        // Validate after merge: a site's `defaults:` may name a collection the
        // theme declares, so the check runs against the merged collection set.
        validate_defaults(&config.defaults, &config.collections)
            .with_context(|| format!("validating `defaults` in {}", path.display()))?;
        // Fill in default `related` weights against the final (merged) taxonomy
        // set: equal weight on every declared taxonomy plus the `links` graph.
        if config.related.weights.is_empty() {
            config.related.weights = default_related_weights(&config.taxonomies);
        }
        // Derive the URL fields once, from the final (possibly theme-merged) map.
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

    /// Load and parse a single `config.yaml` (no theme resolution). Returns the
    /// typed `Config` and the raw `site:` sub-mapping. `defaults:` are parsed but
    /// **not** validated against `collections:` here, and `site_url`/`base_path`
    /// are **not** derived here — both are deferred to
    /// [`load_with_theme`](Self::load_with_theme) so they see a theme's merged
    /// values. A missing file yields defaults and an empty site map.
    fn load(path: &Path) -> Result<(Self, Mapping)> {
        if !path.exists() {
            return Ok((Self::default(), Mapping::new()));
        }
        let source =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let raw: Value = serde_yaml_ng::from_str(&source)
            .with_context(|| format!("parsing {} as YAML", path.display()))?;
        // An empty or comment-only file parses to null (not a mapping); treat it
        // like a missing file so a fully-commented config.yaml still builds.
        if !raw.is_mapping() {
            return Ok((Self::default(), Mapping::new()));
        }
        let mut config: Self = serde_yaml_ng::from_str(&source)
            .with_context(|| format!("parsing {} into Config", path.display()))?;
        let (site, defaults_map, collections_map, taxonomies_map, related_map) = match raw {
            Value::Mapping(mut m) => {
                let site = match m.remove(Value::String("site".into())) {
                    Some(Value::Mapping(s)) => s,
                    _ => Mapping::new(),
                };
                let defaults = match m.remove(Value::String("defaults".into())) {
                    Some(Value::Mapping(d)) => Some(d),
                    _ => None,
                };
                let collections = match m.remove(Value::String("collections".into())) {
                    Some(Value::Mapping(c)) => Some(c),
                    _ => None,
                };
                let taxonomies = match m.remove(Value::String("taxonomies".into())) {
                    Some(Value::Sequence(t)) => Some(t),
                    _ => None,
                };
                let related = match m.remove(Value::String("related".into())) {
                    Some(Value::Mapping(r)) => Some(r),
                    _ => None,
                };
                (site, defaults, collections, taxonomies, related)
            }
            _ => (Mapping::new(), None, None, None, None),
        };
        if let Some(map) = collections_map {
            config.collections = parse_collections(&map)
                .with_context(|| format!("parsing `collections` in {}", path.display()))?;
        }
        if let Some(map) = defaults_map {
            config.defaults = parse_defaults(&map)
                .with_context(|| format!("parsing `defaults` in {}", path.display()))?;
        }
        // Parse the declared taxonomies (an absent block yields none).
        config.taxonomies = taxonomy::parse(taxonomies_map.as_ref())
            .with_context(|| format!("parsing `taxonomies` in {}", path.display()))?;
        if let Some(map) = related_map {
            config.related = parse_related(&map)
                .with_context(|| format!("parsing `related` in {}", path.display()))?;
        }
        Ok((config, site))
    }

    /// Layer a loaded theme beneath this (site) config. The theme's
    /// `templates/`/`archives/`/`static/` overlay beneath the site's (derived on
    /// demand by the `*_roots` helpers — `theme_dir` itself is recorded in
    /// `self.theme`); collections/defaults merge by name and taxonomies by value
    /// with the site winning; the `site:` map is deep-merged with the site
    /// winning. The site's own `*_dir` paths are left untouched, as are
    /// `content`/`output`/`data`.
    fn apply_theme(&mut self, site: &mut Mapping, theme: Config, theme_site: Mapping) {
        // Templates, archives, and static overlay beneath the site via the
        // `*_roots` helpers, which derive the theme's conventional
        // `templates/`/`archives/`/`static/` subdirs from `self.theme`. A theme's
        // own `*_dir` config keys do not apply. The site's `*_dir` fields keep
        // their site values so the site layer still resolves.
        // Collections & defaults: theme entries first, site overrides by name or
        // appends. Taxonomies: theme then site, dedup preserving order.
        self.collections = merge_named(theme.collections, std::mem::take(&mut self.collections));
        self.defaults = merge_named(theme.defaults, std::mem::take(&mut self.defaults));
        let mut taxonomies = theme.taxonomies;
        for t in std::mem::take(&mut self.taxonomies) {
            if !taxonomies.contains(&t) {
                taxonomies.push(t);
            }
        }
        self.taxonomies = taxonomies;
        // A theme may enable the hashtag pass; the site can also enable it.
        self.hashtags = self.hashtags || theme.hashtags;
        // `related`: the site wins wholesale when it sets weights; otherwise it
        // inherits the theme's. Defaults (when both are silent) are synthesized
        // later in `load_with_theme` from the merged taxonomy set.
        if self.related.weights.is_empty() {
            self.related.weights = theme.related.weights;
        }
        // `site:` map: theme as base, site overrides (deep). The caller
        // (`load_with_theme`) derives `site_url`/`base_path` from the merged map.
        let mut merged = theme_site;
        deep_merge(&mut merged, std::mem::take(site));
        *site = merged;
    }

    /// The theme's conventional `<name>/` subdir, if a theme is configured.
    /// Themes always use the conventional subdir name regardless of their own
    /// `*_dir` config keys; this is the single place that knowledge lives.
    fn theme_subdir(&self, name: &str) -> Option<PathBuf> {
        self.theme.as_ref().map(|theme| theme.join(name))
    }

    /// Ordered overlay roots for an asset class: the theme's conventional subdir
    /// (if a theme is configured) followed by the site's own dir. Consumers walk
    /// them in order so the site overlays the theme on path collisions. With no
    /// theme this is just the site dir.
    fn overlay_roots(&self, theme_sub: &str, site_dir: &Path) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(theme_root) = self.theme_subdir(theme_sub) {
            roots.push(theme_root);
        }
        roots.push(site_dir.to_path_buf());
        roots
    }

    /// Ordered `static/` source roots (theme then site). The static-copy phase
    /// walks them so the site overlays the theme. With no theme this is just
    /// `static_dir`.
    pub fn static_roots(&self) -> Vec<PathBuf> {
        self.overlay_roots("static", &self.static_dir)
    }

    /// Ordered `templates/` roots (theme then site). The Tera loader walks them
    /// so a site template overrides a theme template of the same name. With no
    /// theme this is just `templates_dir`.
    pub fn template_roots(&self) -> Vec<PathBuf> {
        self.overlay_roots("templates", &self.templates_dir)
    }

    /// Ordered `archives/` roots (theme then site). The archive phase walks them
    /// so a site archive overrides a theme archive of the same name. With no
    /// theme this is just `archives_dir`.
    pub fn archive_roots(&self) -> Vec<PathBuf> {
        self.overlay_roots("archives", &self.archives_dir)
    }
}

/// Merge two `(name, value)` lists: start from `base`, then for each entry in
/// `overrides` replace the same-named entry in place or append it. Preserves
/// `base` order with overrides' additions at the end; used for collections and
/// defaults so a theme's entries are the base and the site's win by name.
fn merge_named<T>(mut base: Vec<(String, T)>, overrides: Vec<(String, T)>) -> Vec<(String, T)> {
    for (name, value) in overrides {
        if let Some(slot) = base.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = value;
        } else {
            base.push((name, value));
        }
    }
    base
}

/// Walk `roots` in order; key each file kept by `keep` to its path relative to
/// its root. A later root's file replaces an earlier one's under the same key,
/// in place (so the site overlays the theme), preserving first-seen order.
/// Returns `(rel path, full path)` pairs. The filesystem analog of
/// [`merge_named`]: callers feed it the `*_roots()` overlay order and consume the
/// deduplicated pairs (archives parse them, the Tera loader registers them).
pub(crate) fn overlay_files(
    roots: &[PathBuf],
    keep: impl Fn(&Path) -> bool,
) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root) {
            let entry = entry.with_context(|| format!("walking {}", root.display()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if !keep(path) {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .with_context(|| format!("stripping prefix from {}", path.display()))?
                .to_path_buf();
            if let Some(slot) = files.iter_mut().find(|(r, _)| *r == rel) {
                slot.1 = path.to_path_buf();
            } else {
                files.push((rel, path.to_path_buf()));
            }
        }
    }
    Ok(files)
}

/// Recursively merge `over` into `base`: nested mappings merge key-by-key; any
/// other value (or a type mismatch) replaces. Used to layer the site's `site:`
/// map over the theme's, with the site winning.
fn deep_merge(base: &mut Mapping, over: Mapping) {
    for (k, v) in over {
        match v {
            Value::Mapping(om) if matches!(base.get(&k), Some(Value::Mapping(_))) => {
                if let Some(Value::Mapping(bm)) = base.get_mut(&k) {
                    deep_merge(bm, om);
                }
            }
            other => {
                base.insert(k, other);
            }
        }
    }
}

/// Parse the `collections:` mapping into named [`Query`]s, preserving
/// `config.yaml` order. Each value is a query sub-mapping validated by
/// [`Query::from_yaml_mapping`] (same key validation as generator `query:`
/// blocks and the old `query()` function).
fn parse_collections(map: &Mapping) -> Result<Vec<(String, Query)>> {
    let mut collections = Vec::with_capacity(map.len());
    for (key, value) in map {
        let name = key
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("collection names must be strings"))?;
        let query_map = value.as_mapping().ok_or_else(|| {
            anyhow::anyhow!("collection `{}` must be a mapping of query options", name)
        })?;
        let query = Query::from_yaml_mapping(query_map)
            .with_context(|| format!("in collection `{}`", name))?;
        collections.push((name.to_string(), query));
    }
    Ok(collections)
}

/// Parse the `related:` mapping into [`Related`] options: just a `weights:`
/// sub-map of `namespace -> weight` (a taxonomy name, or `links` for the link
/// graph), preserving `config.yaml` order. `limit` and `omit` are *not* config —
/// they are per-call arguments of the `related()` filter — so the parsed
/// `Related` carries only weights and leaves those at their defaults. Unknown
/// top-level keys are rejected so typos fail loudly. An empty/absent `weights`
/// is left empty here; [`Config::load_with_theme`] fills the default set once the
/// taxonomy list is final.
fn parse_related(map: &Mapping) -> Result<Related> {
    for key in map.keys() {
        match key.as_str() {
            Some("weights") => {}
            // `limit` used to live here; it is now a filter argument. Fail with a
            // pointer rather than silently ignoring a stale config.
            Some("limit") => {
                return Err(anyhow::anyhow!(
                    "related: `limit` is no longer a config key — pass it to the \
                     filter instead, e.g. `page.id_path | related(limit=5)`"
                ));
            }
            other => {
                return Err(anyhow::anyhow!(
                    "related: unknown key `{}` (allowed: weights)",
                    other.unwrap_or("<non-string>")
                ));
            }
        }
    }

    let mut weights = Vec::new();
    if let Some(value) = map.get(Value::String("weights".into())) {
        let weights_map = value
            .as_mapping()
            .ok_or_else(|| anyhow::anyhow!("related: `weights` must be a mapping"))?;
        for (key, value) in weights_map {
            let name = key
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("related: weight names must be strings"))?;
            let weight = value.as_f64().ok_or_else(|| {
                anyhow::anyhow!("related: weight for `{}` must be a number", name)
            })?;
            weights.push((name.to_string(), weight));
        }
    }

    Ok(Related {
        weights,
        ..Related::default()
    })
}

/// The zero-config `related` weighting: equal weight `1.0` on every declared
/// taxonomy plus the `links` graph. So a site with no `related:` block still
/// relates by `links`, and by `tags` (etc.) for whatever taxonomies it declares.
fn default_related_weights(taxonomies: &[String]) -> Vec<(String, f64)> {
    let mut weights: Vec<(String, f64)> = taxonomies.iter().map(|t| (t.clone(), 1.0)).collect();
    weights.push((LINKS.to_string(), 1.0));
    weights
}

/// Parse the `defaults:` mapping into `(collection name, default frontmatter)`
/// pairs, preserving `config.yaml` order. Each value is a frontmatter mapping
/// whose values fill keys absent on the collection's members (see
/// `build::defaults`). Names are *not* validated against `collections:` here;
/// [`validate_defaults`] does that after any theme merge.
fn parse_defaults(map: &Mapping) -> Result<Vec<(String, Mapping)>> {
    let mut defaults = Vec::with_capacity(map.len());
    for (key, value) in map {
        let name = key
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("defaults keys must be collection names (strings)"))?;
        let frontmatter = value.as_mapping().ok_or_else(|| {
            anyhow::anyhow!(
                "defaults for `{}` must be a mapping of frontmatter values",
                name
            )
        })?;
        defaults.push((name.to_string(), frontmatter.clone()));
    }
    Ok(defaults)
}

/// Ensure every `defaults:` entry names a declared collection. Run after the
/// theme merge so a site's defaults may reference a theme-defined collection.
fn validate_defaults(
    defaults: &[(String, Mapping)],
    collections: &[(String, Query)],
) -> Result<()> {
    for (name, _) in defaults {
        if !collections.iter().any(|(n, _)| n == name) {
            return Err(anyhow::anyhow!(
                "defaults: `{}` does not name a collection (declare it under `collections:`)",
                name
            ));
        }
    }
    Ok(())
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
    use crate::test_util::{cleanup, tempdir};
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
        let (config, site) = Config::load_with_theme(path).unwrap();
        assert_eq!(config.content_dir, PathBuf::from("content"));
        assert_eq!(config.output_dir, PathBuf::from("public"));
        assert!(site.is_empty());
    }

    #[test]
    fn empty_or_comment_only_file_yields_defaults() {
        let dir = tempdir("config");
        // A file that exists but holds only comments parses to YAML null.
        let path = write_config(&dir, "# just a comment\n");
        let (config, site) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.content_dir, PathBuf::from("content"));
        assert_eq!(config.output_dir, PathBuf::from("public"));
        assert!(config.theme.is_none());
        assert!(site.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn partial_overrides_keep_defaults_for_missing_keys() {
        let dir = tempdir("config");
        let path = write_config(&dir, "output_dir: build\n");
        let (config, site) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.output_dir, PathBuf::from("build"));
        assert_eq!(config.content_dir, PathBuf::from("content"));
        assert!(site.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn site_submap_is_extracted() {
        let dir = tempdir("config");
        let path = write_config(&dir, "site:\n  title: My Site\n  description: A blog\n");
        let (_, site) = Config::load_with_theme(&path).unwrap();
        assert_eq!(
            site.get(Value::String("title".into()))
                .and_then(|v| v.as_str()),
            Some("My Site"),
        );
        cleanup(&dir);
    }

    #[test]
    fn site_absent_yields_empty_map() {
        let dir = tempdir("config");
        let path = write_config(&dir, "content_dir: foo\n");
        let (_, site) = Config::load_with_theme(&path).unwrap();
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
        let dir = tempdir("config");
        let path = write_config(
            &dir,
            "site:\n  url: https://example.com/\n  base_path: blog\n",
        );
        let (config, site) = Config::load_with_theme(&path).unwrap();
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
        let dir = tempdir("config");
        let path = write_config(&dir, "site:\n  title: My Site\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert!(config.site_url.is_none());
        assert_eq!(config.base_path, "");
        cleanup(&dir);
    }

    #[test]
    fn defaults_block_is_parsed() {
        let dir = tempdir("config");
        let path = write_config(
            &dir,
            "collections:\n  posts:\n    path: \"posts/*.md\"\ndefaults:\n  posts:\n    permalink: \":yyyy/:mm/:dd/:slug\"\n",
        );
        let (config, _) = Config::load_with_theme(&path).unwrap();
        // Parsed as a (collection name, frontmatter) pair.
        assert_eq!(config.defaults.len(), 1);
        let (name, fm) = &config.defaults[0];
        assert_eq!(name, "posts");
        assert_eq!(
            fm.get(Value::String("permalink".into()))
                .and_then(|v| v.as_str()),
            Some(":yyyy/:mm/:dd/:slug")
        );
        cleanup(&dir);
    }

    #[test]
    fn defaults_referencing_unknown_collection_errors() {
        let dir = tempdir("config");
        // No `collections:` block → `posts` is unknown.
        let path = write_config(&dir, "defaults:\n  posts:\n    template: post.html\n");
        assert!(Config::load_with_theme(&path).is_err());
        cleanup(&dir);
    }

    #[test]
    fn collections_block_is_parsed() {
        let dir = tempdir("config");
        let path = write_config(
            &dir,
            "collections:\n  posts:\n    path: \"posts/*.md\"\n  recent:\n    order_by: updated\n",
        );
        let (config, _) = Config::load_with_theme(&path).unwrap();
        // Filter out the always-injected `all` collection.
        let names: Vec<&str> = config
            .collections
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| *n != ALL)
            .collect();
        // Order preserved from config.yaml.
        assert_eq!(names, vec!["posts", "recent"]);
        let posts = &config.collections[0].1;
        assert!(posts.path.is_some());
        cleanup(&dir);
    }

    #[test]
    fn collection_limit_key_errors() {
        // `limit` is no longer a collection key; it must fail loudly (the parse
        // error points users to the filter/archive replacements).
        let dir = tempdir("config");
        let path = write_config(
            &dir,
            "collections:\n  posts:\n    path: \"posts/*.md\"\n    limit: 5\n",
        );
        let err = format!("{:#}", Config::load_with_theme(&path).unwrap_err());
        assert!(err.contains("limit"), "error should mention limit: {err}");
        cleanup(&dir);
    }

    #[test]
    fn taxonomies_absent_yields_empty() {
        let dir = tempdir("config");
        let path = write_config(&dir, "content_dir: foo\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert!(config.taxonomies.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn taxonomies_block_parsed_in_declaration_order() {
        let dir = tempdir("config");
        let path = write_config(&dir, "taxonomies:\n  - tags\n  - categories\n  - series\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.taxonomies, vec!["tags", "categories", "series"]);
        cleanup(&dir);
    }

    #[test]
    fn missing_file_yields_empty_taxonomies() {
        let path = Path::new("/definitely/does/not/exist/config.yaml");
        let (config, _) = Config::load_with_theme(path).unwrap();
        assert!(config.taxonomies.is_empty());
    }

    #[test]
    fn related_block_is_parsed() {
        let dir = tempdir("config");
        let path = write_config(
            &dir,
            "taxonomies:\n  - tags\nrelated:\n  weights:\n    tags: 3.0\n    links: 2.0\n",
        );
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(
            config.related.weights,
            vec![("tags".to_string(), 3.0), ("links".to_string(), 2.0)]
        );
        cleanup(&dir);
    }

    #[test]
    fn related_absent_defaults_to_declared_taxonomies_plus_links() {
        let dir = tempdir("config");
        let path = write_config(&dir, "taxonomies:\n  - tags\n  - categories\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        // Equal weight on each declared taxonomy plus the `links` graph.
        assert_eq!(
            config.related.weights,
            vec![
                ("tags".to_string(), 1.0),
                ("categories".to_string(), 1.0),
                ("links".to_string(), 1.0),
            ]
        );
        cleanup(&dir);
    }

    #[test]
    fn related_with_no_taxonomies_defaults_to_links_only() {
        let dir = tempdir("config");
        let path = write_config(&dir, "content_dir: foo\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.related.weights, vec![("links".to_string(), 1.0)]);
        cleanup(&dir);
    }

    #[test]
    fn related_limit_in_config_errors() {
        // `limit` moved to the filter; leaving it in config fails with a pointer.
        let dir = tempdir("config");
        let path = write_config(&dir, "related:\n  weights:\n    tags: 1.0\n  limit: 5\n");
        let err = format!("{:#}", Config::load_with_theme(&path).unwrap_err());
        assert!(err.contains("limit"), "error should mention limit: {err}");
        cleanup(&dir);
    }

    #[test]
    fn related_unknown_key_errors() {
        let dir = tempdir("config");
        let path = write_config(&dir, "related:\n  wieghts:\n    tags: 1.0\n");
        assert!(Config::load_with_theme(&path).is_err());
        cleanup(&dir);
    }

    #[test]
    fn collections_absent_yields_empty() {
        // No `collections:` block declares no *named* collections — but a default
        // `all` is always injected (see `default_all_collection_injected...`), so
        // we assert there is no user collection rather than an empty list.
        let dir = tempdir("config");
        let path = write_config(&dir, "content_dir: foo\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        let named: Vec<&str> = config
            .collections
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| *n != ALL)
            .collect();
        assert!(named.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn default_all_collection_injected_when_absent() {
        use crate::query::{OrderKey, SortDir};
        // With no `collections:` block, an `all` collection is injected with the
        // default query (every doc, date desc) so `all()`/sitemaps have a
        // canonical listing.
        let dir = tempdir("config");
        let path = write_config(&dir, "content_dir: foo\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        let all = config
            .collections
            .iter()
            .find(|(n, _)| n == ALL)
            .map(|(_, q)| q)
            .expect("an `all` collection should always exist");
        assert!(all.path.is_none());
        assert_eq!(all.order_by, OrderKey::Date);
        assert_eq!(all.sort, SortDir::Desc);
        cleanup(&dir);
    }

    #[test]
    fn user_all_collection_overrides_default() {
        use crate::query::OrderKey;
        // A site-declared `all` wins; it is not duplicated by the injected default.
        let dir = tempdir("config");
        let path = write_config(&dir, "collections:\n  all:\n    order_by: title\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        let alls: Vec<&Query> = config
            .collections
            .iter()
            .filter(|(n, _)| n == ALL)
            .map(|(_, q)| q)
            .collect();
        assert_eq!(alls.len(), 1, "no duplicate `all` collection");
        assert_eq!(alls[0].order_by, OrderKey::Title);
        cleanup(&dir);
    }

    #[test]
    fn defaults_may_reference_all_without_declaring_collections() {
        // `all` always exists, so `defaults:` may target it with no `collections:`
        // block — which would otherwise be an unknown-collection error.
        let dir = tempdir("config");
        let path = write_config(&dir, "defaults:\n  all:\n    template: base.html\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.defaults.len(), 1);
        assert_eq!(config.defaults[0].0, "all");
        cleanup(&dir);
    }

    #[test]
    fn collections_unknown_query_key_errors() {
        let dir = tempdir("config");
        let path = write_config(&dir, "collections:\n  bad:\n    paht: x\n");
        assert!(Config::load_with_theme(&path).is_err());
        cleanup(&dir);
    }

    #[test]
    fn defaults_absent_yields_empty() {
        let dir = tempdir("config");
        let path = write_config(&dir, "content_dir: foo\n");
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert!(config.defaults.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn no_config_file_yields_empty_url_fields() {
        let path = Path::new("/definitely/does/not/exist/config.yaml");
        let (config, _) = Config::load_with_theme(path).unwrap();
        assert!(config.site_url.is_none());
        assert_eq!(config.base_path, "");
    }

    /// Write `body` to `dir/config.yaml`, creating `dir` if needed.
    fn write_config_in(dir: &Path, body: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        write_config(dir, body)
    }

    #[test]
    fn theme_overlays_templates_archives_and_static_roots() {
        let dir = tempdir("config");
        let theme = dir.join("themes/demo");
        write_config_in(&theme, "site:\n  title: Demo\n");
        let path = write_config(&dir, &format!("theme: {}\n", theme.display()));
        let (config, _) = Config::load_with_theme(&path).unwrap();
        // Site dirs keep their site values; the theme is overlaid beneath via the
        // `*_roots` helpers (theme first, site last — site wins).
        assert_eq!(config.templates_dir, PathBuf::from("templates"));
        assert_eq!(config.archives_dir, PathBuf::from("archives"));
        assert_eq!(
            config.template_roots(),
            vec![theme.join("templates"), PathBuf::from("templates")]
        );
        assert_eq!(
            config.archive_roots(),
            vec![theme.join("archives"), PathBuf::from("archives")]
        );
        assert_eq!(
            config.static_roots(),
            vec![theme.join("static"), PathBuf::from("static")]
        );
        // content/output/data stay site-only.
        assert_eq!(config.content_dir, PathBuf::from("content"));
        assert_eq!(config.output_dir, PathBuf::from("public"));
        assert_eq!(config.data_dir, PathBuf::from("data"));
        cleanup(&dir);
    }

    #[test]
    fn theme_ignores_its_own_dir_names() {
        let dir = tempdir("config");
        let theme = dir.join("theme");
        // A theme always contributes its conventional subdirs regardless of any
        // `*_dir` keys in its own config.yaml.
        write_config_in(
            &theme,
            "templates_dir: layouts\narchives_dir: views\nstatic_dir: assets\n",
        );
        let path = write_config(&dir, &format!("theme: {}\n", theme.display()));
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.template_roots()[0], theme.join("templates"));
        assert_eq!(config.archive_roots()[0], theme.join("archives"));
        assert_eq!(config.static_roots()[0], theme.join("static"));
        cleanup(&dir);
    }

    #[test]
    fn theme_without_config_resolves_conventional_dirs() {
        let dir = tempdir("config");
        let theme = dir.join("theme");
        fs::create_dir_all(&theme).unwrap(); // no config.yaml inside
        let path = write_config(&dir, &format!("theme: {}\n", theme.display()));
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.template_roots()[0], theme.join("templates"));
        assert_eq!(config.archive_roots()[0], theme.join("archives"));
        assert_eq!(config.static_roots()[0], theme.join("static"));
        cleanup(&dir);
    }

    #[test]
    fn template_and_archive_roots_are_just_site_without_theme() {
        let config = Config::default();
        assert_eq!(config.template_roots(), vec![PathBuf::from("templates")]);
        assert_eq!(config.archive_roots(), vec![PathBuf::from("archives")]);
    }

    #[test]
    fn static_roots_orders_theme_before_site() {
        let dir = tempdir("config");
        let theme = dir.join("theme");
        write_config_in(&theme, "");
        let path = write_config(&dir, &format!("theme: {}\n", theme.display()));
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(
            config.static_roots(),
            vec![theme.join("static"), PathBuf::from("static")]
        );
        cleanup(&dir);
    }

    #[test]
    fn static_roots_is_just_site_without_theme() {
        let config = Config::default();
        assert_eq!(config.static_roots(), vec![PathBuf::from("static")]);
    }

    #[test]
    fn theme_collections_merge_with_site_winning_by_name() {
        use crate::query::OrderKey;
        let dir = tempdir("config");
        let theme = dir.join("theme");
        write_config_in(
            &theme,
            "collections:\n  posts:\n    order_by: title\n  news:\n    order_by: title\n",
        );
        let path = write_config(
            &dir,
            &format!(
                "theme: {}\ncollections:\n  posts:\n    order_by: updated\n",
                theme.display()
            ),
        );
        let (config, _) = Config::load_with_theme(&path).unwrap();
        // Filter out the always-injected `all` collection.
        let names: Vec<&str> = config
            .collections
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| *n != ALL)
            .collect();
        // Theme order preserved; site override keeps the theme's slot.
        assert_eq!(names, vec!["posts", "news"]);
        // Site's `posts` query replaces the theme's by name; theme-only `news` kept.
        assert_eq!(config.collections[0].1.order_by, OrderKey::Updated); // site wins
        assert_eq!(config.collections[1].1.order_by, OrderKey::Title); // theme-only kept
        cleanup(&dir);
    }

    #[test]
    fn theme_taxonomies_merge_and_dedup() {
        let dir = tempdir("config");
        let theme = dir.join("theme");
        write_config_in(&theme, "taxonomies:\n  - tags\n  - categories\n");
        let path = write_config(
            &dir,
            &format!(
                "theme: {}\ntaxonomies:\n  - tags\n  - series\n",
                theme.display()
            ),
        );
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.taxonomies, vec!["tags", "categories", "series"]);
        cleanup(&dir);
    }

    #[test]
    fn site_defaults_may_reference_theme_collection() {
        let dir = tempdir("config");
        let theme = dir.join("theme");
        // Collection lives only in the theme.
        write_config_in(&theme, "collections:\n  posts:\n    path: \"posts/*.md\"\n");
        let path = write_config(
            &dir,
            &format!(
                "theme: {}\ndefaults:\n  posts:\n    template: post.html\n",
                theme.display()
            ),
        );
        // Validates post-merge, so referencing the theme's collection is fine.
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert_eq!(config.defaults.len(), 1);
        assert_eq!(config.defaults[0].0, "posts");
        cleanup(&dir);
    }

    #[test]
    fn theme_site_map_deep_merges_with_site_winning() {
        let dir = tempdir("config");
        let theme = dir.join("theme");
        write_config_in(
            &theme,
            "site:\n  title: Theme Title\n  author: Theme Author\n  url: https://theme.example\n",
        );
        let path = write_config(
            &dir,
            &format!(
                "theme: {}\nsite:\n  title: Site Title\n  url: https://site.example/\n",
                theme.display()
            ),
        );
        let (config, site) = Config::load_with_theme(&path).unwrap();
        // Site key wins; theme-only key kept.
        assert_eq!(
            site.get(Value::String("title".into()))
                .and_then(|v| v.as_str()),
            Some("Site Title")
        );
        assert_eq!(
            site.get(Value::String("author".into()))
                .and_then(|v| v.as_str()),
            Some("Theme Author")
        );
        // site_url re-derived from the merged map (site wins, trailing slash trimmed).
        assert_eq!(config.site_url.as_deref(), Some("https://site.example"));
        cleanup(&dir);
    }

    #[test]
    fn theme_hashtags_enable_propagates() {
        let dir = tempdir("config");
        let theme = dir.join("theme");
        write_config_in(&theme, "hashtags: true\n");
        let path = write_config(&dir, &format!("theme: {}\n", theme.display()));
        let (config, _) = Config::load_with_theme(&path).unwrap();
        assert!(config.hashtags);
        cleanup(&dir);
    }
}
