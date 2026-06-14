//! The archives phase (spec §6 phase 3). An *archive* is a file in `archives/`
//! whose frontmatter declares a `kind` — `collection` or `taxonomy` — naming a
//! collection or taxonomy defined in `config.yaml`. Each archive fans out into
//! one or more **view pages** over docs that already exist:
//!
//! - **collection** — paginate the named collection into list pages.
//! - **taxonomy** — one (optionally paginated) archive page per term.
//!
//! Archives read only the *frozen classification* (an `Arc<DocIndex>` of source
//! docs built by [`crate::build::classify`]); they never read each other's
//! output, so there is no ordering between them and the phase runs as a Rayon
//! sink. The page-1 URL is the archive's `permalink` verbatim; pages ≥2 get a
//! `page/N/` segment (see [`crate::permalink::paginate_pattern`]). Emitted pages
//! are appended to the live index but are *not* re-classified — generated pages
//! never appear in `collection()`/`taxonomy()`.

mod builtin;

use crate::build::markup;
use crate::config::{self, Config, Feed, Sitemap};
use crate::doc::{Doc, DocMeta};
use crate::doc_index::DocIndex;
use crate::permalink;
use crate::query::Query;
use crate::site_data::SiteData;
use crate::tera_env::{MarkupEnv, build_markup_env};
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One file in `archives/`. The body is the Tera template each emitted page
/// renders; `kind` selects what it iterates.
pub struct Archive {
    pub id_path: PathBuf,
    pub kind: ArchiveKind,
    pub per_page: Option<usize>,
    /// Max items this archive paginates over, before `per_page` splits them into
    /// pages. For a collection archive this caps the total; for a taxonomy
    /// archive (paginated once per term) it caps items *per term*. `None` or `0`
    /// means no cap. Since archives reference a collection/taxonomy by name and
    /// can't pass a render-time `limit`, this is where that bound is declared.
    pub limit: Option<usize>,
    pub permalink: String,
    pub template: Option<String>,
    pub body: String,
    /// Verbatim frontmatter, so archive bodies can refer to author-supplied
    /// fields via `{{ page.data.xxx }}`.
    pub data: Mapping,
    /// Optional late-binding query applied per term on `kind: taxonomy` archives:
    /// each term's docs are filtered (path glob + `omit`) and re-ordered before
    /// pagination, so a tag shared across sections can be scoped to one path
    /// (e.g. `path: "posts/**"`). Only valid for taxonomy archives — collection
    /// archives inherit their named collection's `Query`. See `produce`.
    pub query: Option<Query>,
}

/// The two archive kinds, each naming a classification defined in config.
pub enum ArchiveKind {
    Collection { collection: String },
    Taxonomy { taxonomy: String },
}

/// Pagination context for a single emitted page, surfaced to the body and
/// template as `pagination`.
#[derive(Serialize)]
pub struct Pagination {
    pub current: usize,
    pub total: usize,
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
    pub items: Vec<Doc>,
}

/// The current term, surfaced to taxonomy-archive bodies/templates as `term`.
#[derive(Serialize)]
pub struct Term {
    pub slug: String,
    pub text: String,
}

impl Archive {
    /// Parse an archive file. `kind` and `permalink` are required; the kind's
    /// companion key (`collection:` or `taxonomy:`) is required for that kind.
    pub fn parse(id_path: PathBuf, source: &str) -> Result<Archive> {
        let (data, body) = crate::frontmatter::parse(source)?;

        let permalink = data
            .get("permalink")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow!(
                    "archive `{}` missing required `permalink` field",
                    id_path.display()
                )
            })?
            .to_string();

        let per_page = data
            .get("per_page")
            .and_then(Value::as_u64)
            .map(|n| n as usize);
        let limit = data
            .get("limit")
            .and_then(Value::as_u64)
            .map(|n| n as usize);
        let template = data
            .get("template")
            .and_then(Value::as_str)
            .map(str::to_string);

        let kind_str = data.get("kind").and_then(Value::as_str).ok_or_else(|| {
            anyhow!(
                "archive `{}` missing required `kind` field (collection|taxonomy)",
                id_path.display()
            )
        })?;
        let kind = match kind_str {
            "collection" => {
                let collection = required_name(&data, "collection", &id_path)?;
                ArchiveKind::Collection { collection }
            }
            "taxonomy" => {
                let taxonomy = required_name(&data, "taxonomy", &id_path)?;
                ArchiveKind::Taxonomy { taxonomy }
            }
            other => {
                return Err(anyhow!(
                    "archive `{}` has unknown kind `{}` (expected collection|taxonomy)",
                    id_path.display(),
                    other
                ));
            }
        };

        // Optional `query:` sub-mapping. Reuses the collection `Query` parser, so
        // it accepts/validates the same `path`/`order_by`/`sort`/`omit` keys (and
        // gives the same `limit`-moved error). Only meaningful for taxonomy
        // archives; a collection archive's docs already come from its named
        // collection's query, so a `query:` there is a hard error rather than a
        // silent no-op.
        let query = match data.get("query") {
            None => None,
            Some(v) => {
                let m = v.as_mapping().ok_or_else(|| {
                    anyhow!("archive `{}`: `query` must be a mapping", id_path.display())
                })?;
                Some(Query::from_yaml_mapping(m)?)
            }
        };
        if query.is_some() && matches!(kind, ArchiveKind::Collection { .. }) {
            return Err(anyhow!(
                "archive `{}`: `query` is only valid on `kind: taxonomy`; for a \
                 collection archive, define the filtered collection in config.yaml instead",
                id_path.display()
            ));
        }

        Ok(Archive {
            id_path,
            kind,
            per_page,
            limit,
            permalink,
            template,
            body,
            data,
            query,
        })
    }
}

fn required_name(data: &Mapping, key: &str, id_path: &std::path::Path) -> Result<String> {
    data.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow!(
                "archive `{}` of this kind requires a `{}` field naming the {}",
                id_path.display(),
                key,
                key
            )
        })
}

/// Whether `path`'s file name starts with a dot (e.g. `.DS_Store`). Such files
/// are skipped when collecting archives.
fn is_dotfile(path: &Path) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

pub fn run(
    config: &Config,
    site_data: &SiteData,
    classification: &Arc<DocIndex>,
) -> Result<Vec<Doc>> {
    // Walk the archive roots in overlay order (theme then site), deduped per
    // `id_path` so a site archive replaces a theme archive of the same name — the
    // same per-path override the templates and static layers give. Dotfiles are
    // skipped; `id_path` is the path relative to its root.
    let mut archives: Vec<Archive> = Vec::new();
    for (id_path, path) in config::overlay_files(&config.archive_roots(), |p| !is_dotfile(p))? {
        let source =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let a = Archive::parse(id_path, &source)
            .with_context(|| format!("parsing archive {}", path.display()))?;
        archives.push(a);
    }

    // Inject the built-in sitemap/feed archives (config-driven) as a lowest
    // overlay layer: each is added only when no disk archive already owns its
    // `id_path`, so a theme/site `archives/sitemap.xml` or `archives/feed/<name>.xml`
    // transparently overrides it. A disabled `sitemap`/empty `feed` injects nothing.
    if let Sitemap::Collection(collection) = &config.sitemap {
        let id_path = PathBuf::from("sitemap.xml");
        if !archives.iter().any(|a| a.id_path == id_path) {
            archives.push(builtin::sitemap_archive(collection));
        }
    }
    if let Feed::Collections(collections) = &config.feed {
        for collection in collections {
            let id_path = PathBuf::from(format!("feed/{collection}.xml"));
            if !archives.iter().any(|a| a.id_path == id_path) {
                archives.push(builtin::feed_archive(collection));
            }
        }
    }

    if archives.is_empty() {
        return Ok(Vec::new());
    }

    // Frozen `DocMeta` view of the source docs (post-markup) for wikilink
    // resolution and URL filters inside archive bodies.
    let snapshot: Arc<Vec<DocMeta>> = Arc::new(classification.to_doc_metas());
    let markup_env = build_markup_env(config, snapshot)?;

    // Archives are mutually independent (each reads only the frozen
    // classification, none reads another's output), so fan out across Rayon —
    // each worker renders archive bodies with its own `MarkupEnv` clone. The
    // emitted pages are returned for the template phase to render; they are never
    // added to the index (generated pages are not classified).
    let emitted: Vec<Doc> = archives
        .par_iter()
        .map_init(
            || markup_env.clone(),
            |env, archive| produce(env, site_data, classification, archive),
        )
        .collect::<Result<Vec<Vec<Doc>>>>()?
        .into_iter()
        .flatten()
        .collect();

    Ok(emitted)
}

/// Produce every page for one archive: one paginated run for a collection, or
/// one paginated run per term for a taxonomy.
fn produce(
    env: &mut MarkupEnv,
    site_data: &SiteData,
    classification: &DocIndex,
    archive: &Archive,
) -> Result<Vec<Doc>> {
    match &archive.kind {
        ArchiveKind::Collection { collection } => {
            let items: Vec<Doc> = classification.get_collection(collection).cloned().collect();
            paginate(env, site_data, archive, &items, None)
        }
        ArchiveKind::Taxonomy { taxonomy } => {
            let mut out = Vec::new();
            let Some(terms) = classification.get_taxonomy(taxonomy) else {
                return Ok(out);
            };
            for (slug, ids) in terms {
                let mut items: Vec<Doc> = ids
                    .iter()
                    .filter_map(|id| classification.doc(id).cloned())
                    .collect();
                // Apply the archive's late-binding query (path glob + omit, then
                // order/sort) to this term's docs. A term emptied by the filter
                // (e.g. a tag that only appears outside the query's path) emits no
                // page. Without a query, items keep the index's id_path order.
                if let Some(query) = &archive.query {
                    items = query.evaluate(items.iter()).into_iter().cloned().collect();
                    if items.is_empty() {
                        continue;
                    }
                }
                // Display text comes from any member's term bucket; fall back to
                // the slug if (impossibly) absent.
                let text = items
                    .iter()
                    .find_map(|d| d.terms.get(taxonomy).and_then(|b| b.get(slug)).cloned())
                    .unwrap_or_else(|| slug.clone());
                let term = Term {
                    slug: slug.clone(),
                    text,
                };
                out.extend(paginate(env, site_data, archive, &items, Some(term))?);
            }
            Ok(out)
        }
    }
}

/// Paginate `items` into page docs for one archive run. `term` (when present)
/// substitutes `:term` in the permalink and is surfaced to the body/template.
fn paginate(
    env: &mut MarkupEnv,
    site_data: &SiteData,
    archive: &Archive,
    items: &[Doc],
    term: Option<Term>,
) -> Result<Vec<Doc>> {
    let term_slug = term.as_ref().map(|t| t.slug.clone());
    let term_value = term
        .map(|t| serde_yaml_ng::to_value(&t))
        .transpose()
        .context("serializing term context")?;

    // `limit` (when set and > 0) caps the item set *before* pagination; `per_page`
    // then splits the capped set into pages. The two are independent and compose
    // — e.g. limit=100, per_page=5 paginates 100 items into 20 pages. `0` (like
    // `per_page`) means no cap.
    let items = match archive.limit.filter(|n| *n > 0) {
        Some(n) => &items[..items.len().min(n)],
        None => items,
    };

    // per_page=0 or unset → single page with every item.
    let per_page = archive
        .per_page
        .filter(|n| *n > 0)
        .unwrap_or(items.len().max(1));
    let total_pages = if items.is_empty() {
        1
    } else {
        items.len().div_ceil(per_page)
    };

    let url_for = |page: usize| -> String {
        let pattern = permalink::paginate_pattern(&archive.permalink, page);
        permalink::to_url(&permalink::expand(
            &pattern,
            &archive.id_path,
            &epoch(),
            term_slug.as_deref(),
        ))
    };

    let mut pages = Vec::with_capacity(total_pages);
    for page_idx in 0..total_pages {
        let page = page_idx + 1;
        let start = page_idx * per_page;
        let end = ((page_idx + 1) * per_page).min(items.len());
        let page_items: Vec<Doc> = items[start..end].to_vec();

        let pattern = permalink::paginate_pattern(&archive.permalink, page);
        let output_path =
            permalink::expand(&pattern, &archive.id_path, &epoch(), term_slug.as_deref());
        let prev_url = (page > 1).then(|| url_for(page - 1));
        let next_url = (page < total_pages).then(|| url_for(page + 1));

        let pagination = Pagination {
            current: page,
            total: total_pages,
            prev_url,
            next_url,
            items: page_items,
        };

        let mut data = archive.data.clone();
        data.insert(
            Value::String("pagination".into()),
            serde_yaml_ng::to_value(&pagination).context("serializing pagination context")?,
        );
        if let Some(term_value) = &term_value {
            data.insert(Value::String("term".into()), term_value.clone());
        }

        let mut doc = Doc {
            id_path: output_path.clone(),
            output_path,
            template: archive.template.clone(),
            title: String::new(),
            summary: String::new(),
            draft: false,
            content: archive.body.clone(),
            terms: std::collections::BTreeMap::new(),
            date: epoch(),
            updated: epoch(),
            aliases: Vec::new(),
            data,
            links: Vec::new(),
        };

        markup::render(env, site_data, &mut doc)?;
        pages.push(doc);
    }
    Ok(pages)
}

fn epoch() -> DateTime<Utc> {
    DateTime::<Utc>::UNIX_EPOCH
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::tempdir;

    #[test]
    fn parse_collection_archive() {
        let source = "---\nkind: collection\ncollection: posts\npermalink: /blog/\nper_page: 2\nlimit: 6\ntemplate: blog.html\n---\nBODY";
        let a = Archive::parse(PathBuf::from("blog.html"), source).unwrap();
        assert_eq!(a.permalink, "/blog/");
        assert_eq!(a.per_page, Some(2));
        assert_eq!(a.limit, Some(6));
        assert_eq!(a.template.as_deref(), Some("blog.html"));
        assert_eq!(a.body, "BODY");
        match a.kind {
            ArchiveKind::Collection { collection } => assert_eq!(collection, "posts"),
            _ => panic!("expected collection kind"),
        }
    }

    #[test]
    fn parse_taxonomy_archive() {
        let source = "---\nkind: taxonomy\ntaxonomy: tags\npermalink: /tags/:term/\n---\nBODY";
        let a = Archive::parse(PathBuf::from("tags.html"), source).unwrap();
        assert_eq!(a.permalink, "/tags/:term/");
        assert_eq!(a.per_page, None);
        // `limit` is optional and absent here.
        assert_eq!(a.limit, None);
        match a.kind {
            ArchiveKind::Taxonomy { taxonomy } => assert_eq!(taxonomy, "tags"),
            _ => panic!("expected taxonomy kind"),
        }
    }

    #[test]
    fn parse_missing_kind_errors() {
        let source = "---\npermalink: /blog/\n---\nBODY";
        assert!(Archive::parse(PathBuf::from("x.html"), source).is_err());
    }

    #[test]
    fn parse_missing_permalink_errors() {
        let source = "---\nkind: collection\ncollection: posts\n---\nBODY";
        assert!(Archive::parse(PathBuf::from("x.html"), source).is_err());
    }

    #[test]
    fn parse_unknown_kind_errors() {
        let source = "---\nkind: all\npermalink: /sitemap.xml\n---\nBODY";
        assert!(Archive::parse(PathBuf::from("x.html"), source).is_err());
    }

    #[test]
    fn parse_collection_kind_requires_collection_name() {
        let source = "---\nkind: collection\npermalink: /blog/\n---\nBODY";
        assert!(Archive::parse(PathBuf::from("x.html"), source).is_err());
    }

    #[test]
    fn parse_taxonomy_kind_requires_taxonomy_name() {
        let source = "---\nkind: taxonomy\npermalink: /tags/:term/\n---\nBODY";
        assert!(Archive::parse(PathBuf::from("x.html"), source).is_err());
    }

    #[test]
    fn parse_taxonomy_archive_with_query() {
        let source = "---\nkind: taxonomy\ntaxonomy: tags\npermalink: /posts/tags/:term/\nquery:\n  path: \"posts/**\"\n  order_by: title\n  sort: asc\n---\nBODY";
        let a = Archive::parse(PathBuf::from("tags.html"), source).unwrap();
        let q = a.query.expect("query should be parsed");
        assert!(q.path.is_some());
        assert_eq!(q.order_by, crate::query::OrderKey::Title);
        assert_eq!(q.sort, crate::query::SortDir::Asc);
    }

    #[test]
    fn parse_collection_archive_with_query_errors() {
        // `query` is taxonomy-only; on a collection archive it is a hard error,
        // not a silent no-op.
        let source = "---\nkind: collection\ncollection: posts\npermalink: /blog/\nquery:\n  path: \"posts/**\"\n---\nBODY";
        assert!(Archive::parse(PathBuf::from("blog.html"), source).is_err());
    }

    #[test]
    fn parse_taxonomy_archive_with_bad_query_key_errors() {
        // Unknown query keys are rejected by the reused `Query` parser.
        let source = "---\nkind: taxonomy\ntaxonomy: tags\npermalink: /tags/:term/\nquery:\n  paht: x\n---\nBODY";
        assert!(Archive::parse(PathBuf::from("tags.html"), source).is_err());
    }

    fn write_archive(base: &std::path::Path, layer: &str, rel: &str, body: &str) {
        let path = base.join(layer).join("archives").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn site_archive_overrides_theme_archive_of_same_name() {
        let base = tempdir("overlay");
        // Both layers define archives/blog.html over the (empty) `posts`
        // collection, with distinct permalinks so the winner is identifiable.
        write_archive(
            &base,
            "theme",
            "blog.html",
            "---\nkind: collection\ncollection: posts\npermalink: /theme-blog/\n---\nBODY",
        );
        write_archive(
            &base,
            "site",
            "blog.html",
            "---\nkind: collection\ncollection: posts\npermalink: /site-blog/\n---\nBODY",
        );
        let config = Config {
            archives_dir: base.join("site").join("archives"),
            theme: Some(base.join("theme")),
            // No templates dir → empty Tera markup env, fine for a body of "BODY".
            templates_dir: base.join("none"),
            ..Config::default()
        };
        let site_data = SiteData {
            site: Mapping::new(),
            data: Mapping::new(),
        };
        let classification = Arc::new(DocIndex::new());
        let pages = run(&config, &site_data, &classification).unwrap();
        // Site's blog.html shadows the theme's: one page, at the site permalink.
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].output_path, PathBuf::from("site-blog/index.html"));
        let _ = fs::remove_dir_all(&base);
    }

    /// A doc tagged with a single `tags` term, for taxonomy-archive tests.
    fn tagged_doc(id_path: &str, term: &str) -> Doc {
        use std::collections::BTreeMap as Map;
        let mut terms: Map<String, Map<String, String>> = Map::new();
        let mut bucket = Map::new();
        bucket.insert(term.to_string(), term.to_string());
        terms.insert("tags".to_string(), bucket);
        Doc {
            id_path: PathBuf::from(id_path),
            output_path: PathBuf::from(id_path).with_extension("html"),
            terms,
            ..Default::default()
        }
    }

    #[test]
    fn taxonomy_archive_query_scopes_terms_by_glob_and_skips_emptied_terms() {
        let base = tempdir("tax-query");
        // `shared` spans posts/ and notes/; `notesonly` lives only under notes/.
        let mut idx = DocIndex::new();
        idx.insert(tagged_doc("posts/a.md", "shared"));
        idx.insert(tagged_doc("notes/b.md", "shared"));
        idx.insert(tagged_doc("notes/c.md", "notesonly"));
        idx.define_taxonomies(&["tags".to_string()]);

        // Archive scoped to posts/**; body lists each page's item id_paths.
        write_archive(
            &base,
            "site",
            "tags.html",
            "---\nkind: taxonomy\ntaxonomy: tags\npermalink: /posts/tags/:term/\nquery:\n  path: \"posts/**\"\n---\n{% for d in pagination.items %}{{ d.id_path }};{% endfor %}",
        );
        let config = Config {
            archives_dir: base.join("site").join("archives"),
            templates_dir: base.join("none"),
            ..Config::default()
        };
        let site_data = SiteData {
            site: Mapping::new(),
            data: Mapping::new(),
        };
        let pages = run(&config, &site_data, &Arc::new(idx)).unwrap();

        // `shared` gets a page; `notesonly` is emptied by the glob → skipped.
        let outputs: Vec<String> = pages
            .iter()
            .map(|p| p.output_path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(outputs, vec!["posts/tags/shared/index.html".to_string()]);
        // That page lists only the posts/ doc, not the notes/ one.
        assert_eq!(pages[0].content.trim(), "posts/a.md;");
        let _ = fs::remove_dir_all(&base);
    }

    /// A minimal source doc at `id_path`, output to `<stem>.html`.
    fn source_doc(id_path: &str) -> Doc {
        Doc {
            id_path: PathBuf::from(id_path),
            output_path: PathBuf::from(id_path).with_extension("html"),
            ..Default::default()
        }
    }

    /// A `DocIndex` of two docs with a classified `all` collection, for the
    /// built-in sitemap/feed tests.
    fn index_with_all() -> DocIndex {
        let mut idx = DocIndex::new();
        idx.insert(source_doc("posts/a.md"));
        idx.insert(source_doc("posts/b.md"));
        idx.define_collection(config::ALL, &Query::default());
        idx
    }

    fn empty_site_data() -> SiteData {
        SiteData {
            site: Mapping::new(),
            data: Mapping::new(),
        }
    }

    fn output_paths(pages: &[Doc]) -> Vec<String> {
        pages
            .iter()
            .map(|p| p.output_path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn builtin_sitemap_and_feed_emitted_when_enabled() {
        let base = tempdir("builtin-on");
        let config = Config {
            archives_dir: base.join("site").join("archives"), // does not exist
            templates_dir: base.join("none"),
            sitemap: Sitemap::Collection(config::ALL.to_string()),
            feed: Feed::Collections(vec![config::ALL.to_string()]),
            ..Config::default()
        };
        let pages = run(&config, &empty_site_data(), &Arc::new(index_with_all())).unwrap();
        let outputs = output_paths(&pages);
        assert!(outputs.contains(&"sitemap.xml".to_string()), "{outputs:?}");
        assert!(outputs.contains(&"feed/all.xml".to_string()), "{outputs:?}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn disk_sitemap_overrides_builtin() {
        let base = tempdir("builtin-override");
        // A site archive named sitemap.xml (its own permalink) shadows the
        // built-in: same `id_path`, so no built-in sitemap is injected.
        write_archive(
            &base,
            "site",
            "sitemap.xml",
            "---\nkind: collection\ncollection: all\npermalink: /custom-sitemap.xml\n---\nBODY",
        );
        let config = Config {
            archives_dir: base.join("site").join("archives"),
            templates_dir: base.join("none"),
            sitemap: Sitemap::Collection(config::ALL.to_string()),
            feed: Feed::Collections(Vec::new()), // feeds disabled for this test
            ..Config::default()
        };
        let pages = run(&config, &empty_site_data(), &Arc::new(index_with_all())).unwrap();
        let outputs = output_paths(&pages);
        assert!(
            outputs.contains(&"custom-sitemap.xml".to_string()),
            "{outputs:?}"
        );
        assert!(!outputs.contains(&"sitemap.xml".to_string()), "{outputs:?}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn disabled_sitemap_and_empty_feed_emit_nothing() {
        let base = tempdir("builtin-off");
        let config = Config {
            archives_dir: base.join("site").join("archives"), // does not exist
            templates_dir: base.join("none"),
            sitemap: Sitemap::Disabled,
            feed: Feed::Collections(Vec::new()),
            ..Config::default()
        };
        let pages = run(&config, &empty_site_data(), &Arc::new(index_with_all())).unwrap();
        assert!(pages.is_empty(), "{:?}", output_paths(&pages));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn theme_only_archive_is_produced() {
        let base = tempdir("theme-only");
        write_archive(
            &base,
            "theme",
            "blog.html",
            "---\nkind: collection\ncollection: posts\npermalink: /blog/\n---\nBODY",
        );
        let config = Config {
            archives_dir: base.join("site").join("archives"), // does not exist
            theme: Some(base.join("theme")),
            templates_dir: base.join("none"),
            ..Config::default()
        };
        let site_data = SiteData {
            site: Mapping::new(),
            data: Mapping::new(),
        };
        let classification = Arc::new(DocIndex::new());
        let pages = run(&config, &site_data, &classification).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].output_path, PathBuf::from("blog/index.html"));
        let _ = fs::remove_dir_all(&base);
    }
}
