use chrono::{DateTime, Datelike, Utc};
use std::path::{Path, PathBuf};

/// Default render location: mirror `id_path` with an `.html` extension.
/// Used when a doc declares no `permalink:` field in frontmatter.
pub fn default_for(id_path: &Path) -> PathBuf {
    id_path.with_extension("html")
}

/// Expand a `permalink:` pattern (spec §5.1) into a path relative to the
/// output dir. Supported variables: `:slug`, `:yyyy`, `:mm`, `:dd`, `:page`.
/// A leading `/` is stripped; a trailing `/` means "write `index.html` here".
/// `page` is `Some(n)` for generator-emitted pages and `None` for authored
/// docs — when `None`, a literal `:page` in the pattern is left untouched.
pub fn expand(pattern: &str, id_path: &Path, date: &DateTime<Utc>, page: Option<usize>) -> PathBuf {
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
    if let Some(n) = page {
        expanded = expanded.replace(":page", &n.to_string());
    }
    let mut path = PathBuf::from(&expanded);
    if trailing_slash {
        path.push("index.html");
    }
    path
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
        let p = expand("/blog/:slug/", Path::new("posts/Hello World.md"), &date(2025, 1, 1), None);
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
        let p = expand("/blog/:slug/", Path::new("hello.md"), &date(2025, 10, 31), None);
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
    fn expand_substitutes_page() {
        let p = expand("/blog/page-:page/", Path::new("blog.html"), &date(1970, 1, 1), Some(3));
        assert_eq!(p, PathBuf::from("blog/page-3/index.html"));
    }

    #[test]
    fn expand_leaves_page_literal_when_none() {
        let p = expand("/blog/page-:page/", Path::new("blog.html"), &date(1970, 1, 1), None);
        assert_eq!(p, PathBuf::from("blog/page-:page/index.html"));
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
}
