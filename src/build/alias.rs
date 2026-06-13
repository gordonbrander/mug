//! Alias redirect stubs. For every `aliases:` entry on a doc, emit a tiny HTML
//! page at the old URL that redirects to the doc's canonical URL — the static
//! equivalent of a 301, mirroring Hugo's `aliases:`. Lets a garden reorganize
//! freely without 404-ing links the outside world already holds.
//!
//! This stage is deliberately self-contained: the stub is a fixed built-in (no
//! user/theme override), so it needs no Tera env, and it does **no** collision
//! handling — that is centralized in [`write::run`](super::write::run), which is
//! first-writer-wins. Because the pipeline appends these stubs after every real
//! page, an alias can never clobber a real page or an archive page.

use crate::build::Output;
use crate::config::Config;
use crate::doc::Doc;
use crate::doc_index::DocIndex;
use crate::permalink;
use anyhow::Result;
use rayon::prelude::*;

/// Emit one redirect stub per `aliases:` entry across the frozen index. Stubs are
/// mutually independent, so the work fans out over Rayon; the emitted outputs are
/// in sorted-`id_path` order (then alias order), which makes the write-time
/// collision tiebreak deterministic.
pub fn run(config: &Config, index: &DocIndex) -> Result<Vec<Output>> {
    let outputs = index
        .par_docs()
        .filter(|doc| !doc.aliases.is_empty())
        .flat_map(|doc| {
            doc.aliases
                .par_iter()
                .filter_map(move |alias| make_stub(config, doc, alias))
        })
        .collect();
    Ok(outputs)
}

/// Build the redirect stub for a single alias, or `None` (with a warning) if the
/// alias is empty/whitespace-only or escapes the output dir.
fn make_stub(config: &Config, doc: &Doc, alias: &str) -> Option<Output> {
    let Some(output_path) = permalink::alias_output_path(alias) else {
        eprintln!(
            "skipping invalid alias '{}' in {}",
            alias,
            doc.id_path.display()
        );
        return None;
    };

    // Canonical URL of the doc, computed exactly like the `link`/`permalink`
    // Tera filters: root-relative is `base_path + to_url`; the absolute form
    // (for `rel=canonical`) prefixes `site.url` when it is set.
    let url = permalink::to_url(&doc.output_path);
    let target = format!("{}{}", config.base_path, url);
    let target_absolute = match &config.site_url {
        Some(site_url) => format!("{site_url}{}{url}", config.base_path),
        None => target.clone(),
    };

    Some(Output {
        output_path,
        content: render_stub(&doc.title, &target, &target_absolute),
        id_path: doc.id_path.clone(),
    })
}

/// The built-in redirect page, kept close to Hugo's `_internal/alias.html`: a
/// `rel=canonical` link (what crawlers consolidate on — no `noindex`), a
/// `<meta refresh>` for no-JS browsers, and a JS `location.replace` that also
/// carries the URL fragment over (so `/old/#heading` deep-links survive).
fn render_stub(title: &str, target: &str, target_absolute: &str) -> String {
    let title = escape_html(title);
    let target_attr = escape_html(target);
    let canonical_attr = escape_html(target_absolute);
    let target_json = json_string(target);
    format!(
        "<!doctype html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <title>{title}</title>\n\
         <link rel=\"canonical\" href=\"{canonical_attr}\">\n\
         <meta http-equiv=\"refresh\" content=\"0; url={target_attr}\">\n\
         </head>\n\
         <body>\n\
         <p>This page has moved to <a href=\"{target_attr}\">{target_attr}</a>.</p>\n\
         <script>location.replace({target_json} + location.hash);</script>\n\
         </body>\n\
         </html>\n",
    )
}

/// Escape a string for use in HTML text or a double/single-quoted attribute.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    out
}

/// Encode a string as a JSON string literal, safe to embed inside a `<script>`
/// (`<`, `>`, `&` are `\u`-escaped so the URL can't break out of the element).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_contains_refresh_canonical_and_js() {
        let html = render_stub("My Post", "/blog/new/", "https://x.com/blog/new/");
        assert!(html.contains("<meta http-equiv=\"refresh\" content=\"0; url=/blog/new/\">"));
        assert!(html.contains("<link rel=\"canonical\" href=\"https://x.com/blog/new/\">"));
        assert!(html.contains("location.replace(\"/blog/new/\" + location.hash);"));
        assert!(html.contains("<title>My Post</title>"));
    }

    #[test]
    fn stub_escapes_title_and_target() {
        let html = render_stub("A & B <tag>", "/a&b/", "/a&b/");
        assert!(html.contains("<title>A &amp; B &lt;tag&gt;</title>"));
        assert!(html.contains("href=\"/a&amp;b/\""));
        // The JS string escapes `&` so it cannot break out of <script>.
        assert!(html.contains("location.replace(\"/a\\u0026b/\" + location.hash);"));
    }
}
