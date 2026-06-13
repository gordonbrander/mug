use chrono::{DateTime, Datelike, Utc};
use std::path::{Component, Path, PathBuf};

/// Default render location: mirror `id_path` with an `.html` extension.
/// Used when a doc declares no `permalink:` field in frontmatter.
pub fn default_for(id_path: &Path) -> PathBuf {
    id_path.with_extension("html")
}

/// Expand a `permalink:` pattern (spec §5.1) into a path relative to the
/// output dir. Supported variables: `:slug`, `:yyyy`, `:mm`, `:dd`, `:term`.
/// A leading `/` is stripped; a trailing `/` means "write `index.html` here".
/// `term` is `Some(slug)` for taxonomy archive pages and `None` otherwise —
/// when `None`, a literal `:term` in the pattern is left untouched.
pub fn expand(pattern: &str, id_path: &Path, date: &DateTime<Utc>, term: Option<&str>) -> PathBuf {
    let stem = id_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let slug = slug::slugify(stem);
    let trailing_slash = pattern.ends_with('/');
    let mut expanded = pattern
        .trim_start_matches('/')
        .replace(":slug", &slug)
        .replace(":yyyy", &format!("{:04}", date.year()))
        .replace(":mm", &format!("{:02}", date.month()))
        .replace(":dd", &format!("{:02}", date.day()));
    if let Some(term) = term {
        expanded = expanded.replace(":term", term);
    }
    let mut path = PathBuf::from(&expanded);
    if trailing_slash {
        path.push("index.html");
    }
    path
}

/// Apply the pagination convention to a landing `permalink` pattern: page 1 is
/// the landing pattern unchanged; pages ≥2 get a `page/N/` segment appended
/// (Hugo-style — `/blog/` → `/blog/page/2/`). The result still contains any
/// `:term`/`:slug` tokens and is meant to be passed to [`expand`]. This is what
/// keeps paginated pages 2+ off the page-1 landing URL (and out of sitemaps,
/// since only landing pages are linked/classified).
pub fn paginate_pattern(permalink: &str, page: usize) -> String {
    if page <= 1 {
        permalink.to_string()
    } else {
        format!("{}/page/{}/", permalink.trim_end_matches('/'), page)
    }
}

/// Convert a filesystem `output_path` into a web URL. Used for `href`
/// values — pagination prev/next URLs today, and the Phase 9 `permalink`
/// filter later.
///
/// - `index.html`            → `/`
/// - `foo/bar/index.html`    → `/foo/bar/`
/// - `posts/p1.html`         → `/posts/p1.html`
/// - `sitemap.xml`           → `/sitemap.xml`
pub fn to_url(output_path: &Path) -> String {
    let s = output_path.to_string_lossy();
    if let Some(stripped) = s.strip_suffix("index.html") {
        let trimmed = stripped.trim_end_matches('/');
        if trimmed.is_empty() {
            "/".to_string()
        } else {
            format!("/{}/", trimmed)
        }
    } else {
        format!("/{}", s)
    }
}

/// Map an `aliases:` entry (a literal historical URL) to the output file its
/// redirect stub is written at, following italic's trailing convention extended
/// with Hugo's extension rule:
///
/// - `/old-url/`          → `old-url/index.html`  (trailing slash)
/// - `/old-url`           → `old-url/index.html`  (no extension)
/// - `/posts/legacy.html` → `posts/legacy.html`   (literal file)
/// - `/feed.xml`          → `feed.xml`
///
/// Unlike [`expand`], **no** `:slug`/`:yyyy` token expansion happens — aliases
/// are literal URLs, so a stray `:` is left untouched. Returns `None` for an
/// empty/whitespace-only alias or one that escapes the output dir via `..`.
pub fn alias_output_path(alias: &str) -> Option<PathBuf> {
    let trimmed = alias.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Note the trailing slash before stripping the leading one.
    let ends_slash = trimmed.ends_with('/');
    let mut path = PathBuf::from(trimmed.trim_start_matches('/'));
    if ends_slash || path.extension().is_none() {
        path.push("index.html");
    }
    // Must stay inside `output_dir`: reject any `..` segment.
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return None;
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn date(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    #[test]
    fn default_for_swaps_extension_to_html() {
        assert_eq!(
            default_for(Path::new("blog/post.md")),
            PathBuf::from("blog/post.html"),
        );
    }

    #[test]
    fn expand_substitutes_slug_from_stem() {
        let p = expand(
            "/blog/:slug/",
            Path::new("posts/Hello World.md"),
            &date(2025, 1, 1),
            None,
        );
        assert_eq!(p, PathBuf::from("blog/hello-world/index.html"));
    }

    #[test]
    fn expand_zero_pads_date_components() {
        let p = expand(
            "/:yyyy/:mm/:dd/:slug.html",
            Path::new("hi.md"),
            &date(2025, 3, 7),
            None,
        );
        assert_eq!(p, PathBuf::from("2025/03/07/hi.html"));
    }

    #[test]
    fn expand_trailing_slash_appends_index_html() {
        let p = expand(
            "/blog/:slug/",
            Path::new("hello.md"),
            &date(2025, 10, 31),
            None,
        );
        assert_eq!(p, PathBuf::from("blog/hello/index.html"));
    }

    #[test]
    fn expand_no_trailing_slash_is_verbatim() {
        let p = expand("/feed.xml", Path::new("rss.md"), &date(2025, 1, 1), None);
        assert_eq!(p, PathBuf::from("feed.xml"));
    }

    #[test]
    fn expand_strips_leading_slash() {
        let p = expand("/:slug.html", Path::new("hi.md"), &date(2025, 1, 1), None);
        assert_eq!(p, PathBuf::from("hi.html"));
    }

    #[test]
    fn expand_handles_no_leading_slash() {
        let p = expand(":slug.html", Path::new("hi.md"), &date(2025, 1, 1), None);
        assert_eq!(p, PathBuf::from("hi.html"));
    }

    #[test]
    fn expand_substitutes_term() {
        let p = expand(
            "/tags/:term/",
            Path::new("tags.html"),
            &date(1970, 1, 1),
            Some("rust"),
        );
        assert_eq!(p, PathBuf::from("tags/rust/index.html"));
    }

    #[test]
    fn expand_leaves_term_literal_when_none() {
        let p = expand(
            "/tags/:term/",
            Path::new("tags.html"),
            &date(1970, 1, 1),
            None,
        );
        assert_eq!(p, PathBuf::from("tags/:term/index.html"));
    }

    #[test]
    fn paginate_pattern_page_one_is_verbatim() {
        assert_eq!(paginate_pattern("/blog/", 1), "/blog/");
    }

    #[test]
    fn paginate_pattern_appends_page_segment() {
        assert_eq!(paginate_pattern("/blog/", 2), "/blog/page/2/");
        assert_eq!(paginate_pattern("/tags/:term/", 3), "/tags/:term/page/3/");
    }

    #[test]
    fn to_url_index_html_root() {
        assert_eq!(to_url(Path::new("index.html")), "/");
    }

    #[test]
    fn to_url_index_html_nested() {
        assert_eq!(to_url(Path::new("blog/page-1/index.html")), "/blog/page-1/");
    }

    #[test]
    fn to_url_non_index() {
        assert_eq!(to_url(Path::new("posts/p1.html")), "/posts/p1.html");
    }

    #[test]
    fn to_url_top_level_xml() {
        assert_eq!(to_url(Path::new("sitemap.xml")), "/sitemap.xml");
    }

    #[test]
    fn alias_output_path_trailing_slash_appends_index() {
        assert_eq!(
            alias_output_path("/old-url/"),
            Some(PathBuf::from("old-url/index.html")),
        );
    }

    #[test]
    fn alias_output_path_no_extension_appends_index() {
        assert_eq!(
            alias_output_path("/old-url"),
            Some(PathBuf::from("old-url/index.html")),
        );
    }

    #[test]
    fn alias_output_path_literal_file() {
        assert_eq!(
            alias_output_path("/posts/legacy.html"),
            Some(PathBuf::from("posts/legacy.html")),
        );
        assert_eq!(
            alias_output_path("/feed.xml"),
            Some(PathBuf::from("feed.xml")),
        );
    }

    #[test]
    fn alias_output_path_strips_leading_slash() {
        assert_eq!(
            alias_output_path("old-url/"),
            Some(PathBuf::from("old-url/index.html")),
        );
    }

    #[test]
    fn alias_output_path_leaves_colon_literal() {
        // No `:slug` token expansion — an old URL with a `:` stays verbatim.
        assert_eq!(
            alias_output_path("/2019/:weird/"),
            Some(PathBuf::from("2019/:weird/index.html")),
        );
    }

    #[test]
    fn alias_output_path_rejects_parent_dir() {
        assert_eq!(alias_output_path("/../escape/"), None);
        assert_eq!(alias_output_path("/a/../../b"), None);
    }

    #[test]
    fn alias_output_path_rejects_empty() {
        assert_eq!(alias_output_path(""), None);
        assert_eq!(alias_output_path("   "), None);
    }
}
