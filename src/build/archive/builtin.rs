//! Built-in archives italic injects when a site/theme hasn't supplied its own.
//! These back the `sitemap`/`feed` config keys (see [`crate::config::Sitemap`] /
//! [`crate::config::Feed`]): the archive phase ([`super::run`]) adds a built-in
//! only when no disk archive already owns its `id_path`, so they act as a lowest
//! overlay layer that a `archives/sitemap.xml` or `archives/feed/<name>.xml`
//! transparently overrides.
//!
//! The bodies mirror the hand-written recipes in `docs/guides/archives.md`. The
//! `permalink` filter already yields an absolute URL (origin + base path), so
//! `<loc>`/`<link>` use it directly — chaining `absolute_url` would re-prefix an
//! already-absolute URL. The channel self-link uses `absolute_url` on `""`.

use super::{Archive, ArchiveKind};
use serde_yaml_ng::Mapping;
use std::path::PathBuf;

/// Default cap on items in a built-in feed (most recent N, per the collection's
/// own order). Sitemaps are uncapped — they should list every page.
const DEFAULT_FEED_LIMIT: usize = 20;

/// `sitemap.xml` body: a `<urlset>` over every page in the covered collection.
const SITEMAP_BODY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{% for doc in pagination.items %}  <url><loc>{{ doc.id_path | permalink }}</loc><lastmod>{{ doc.updated | date(format="%Y-%m-%d") }}</lastmod></url>
{% endfor %}</urlset>
"#;

/// RSS 2.0 feed body over the covered collection (capped by `limit`).
const FEED_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0">
<channel>
  <title>{{ site.title | default(value="") }}</title>
  <link>{{ "" | absolute_url }}</link>
  <description>{{ site.description | default(value="") }}</description>
{% for post in pagination.items %}  <item>
    <title>{{ post.title }}</title>
    <link>{{ post.id_path | permalink }}</link>
    <guid>{{ post.id_path | permalink }}</guid>
    <pubDate>{{ post.date | date(format="%a, %d %b %Y %H:%M:%S +0000") }}</pubDate>
    <description>{{ post.content | striptags | truncate_words(length=80) }}</description>
  </item>
{% endfor %}</channel>
</rss>
"#;

/// Built-in `sitemap.xml` archive over `collection`, output at `/sitemap.xml`.
pub fn sitemap_archive(collection: &str) -> Archive {
    Archive {
        id_path: PathBuf::from("sitemap.xml"),
        kind: ArchiveKind::Collection {
            collection: collection.to_string(),
        },
        per_page: None,
        limit: None,
        permalink: "/sitemap.xml".to_string(),
        template: None,
        body: SITEMAP_BODY.to_string(),
        data: Mapping::new(),
        query: None,
    }
}

/// Built-in feed archive over `collection`, output at `/feed/<collection>.xml`.
pub fn feed_archive(collection: &str) -> Archive {
    Archive {
        id_path: PathBuf::from(format!("feed/{collection}.xml")),
        kind: ArchiveKind::Collection {
            collection: collection.to_string(),
        },
        per_page: None,
        limit: Some(DEFAULT_FEED_LIMIT),
        permalink: format!("/feed/{collection}.xml"),
        template: None,
        body: FEED_BODY.to_string(),
        data: Mapping::new(),
        query: None,
    }
}
