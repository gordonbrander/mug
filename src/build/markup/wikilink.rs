use crate::doc::{Doc, DocMeta};
use crate::html;
use crate::permalink;
use comrak::nodes::{AstNode, NodeValue};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Group docs by the slugified file-stem of their `id_path` — the key a
/// wikilink target is matched against (spec §8). Built once per markup env so
/// each `resolve` is a hash lookup over a small candidate set rather than a
/// full re-slugifying scan of every doc (was O(N²·W) over a build). The key
/// derivation mirrors `resolve`'s candidate filter exactly, including the
/// `unwrap_or("")` fallback, so grouping is behavior-preserving.
pub fn build_stem_index(docs: &[DocMeta]) -> HashMap<String, Vec<DocMeta>> {
    let mut index: HashMap<String, Vec<DocMeta>> = HashMap::new();
    for doc in docs {
        let stem = doc
            .id_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        index.entry(slug::slugify(stem)).or_default().push(doc.clone());
    }
    index
}

/// Resolve `[[Wiki Link]]` / `[[Wiki Link|Display]]` nodes in a parsed comrak
/// AST, in place. comrak tokenizes the `[[…]]` syntax (the
/// `wikilinks_title_after_pipe` extension), so this pass only resolves each
/// target and rewrites the node. Because parsing happens first, wikilinks are
/// never produced inside code spans/blocks — so `[[…]]` written in a fence no
/// longer leaks into a link (the wart of the old pre-parse string scan).
///
/// Each `WikiLink` node is collapsed into an `HtmlInline` carrying either
/// `<a class="wikilink" href="…">display</a>` (resolved) or
/// `<span class="nolink">display</span>` (unresolved, logged to stderr) — the
/// same markup the previous scanner emitted, rendered verbatim because the
/// markup env sets `render.unsafe_`.
///
/// Returns the deduplicated list of resolved target `id_path`s — the source
/// doc's links (consumed by Phase 10 backlinks). This is the one piece of
/// cross-doc state the markup phase produces, so the caller must assign it to
/// `doc.links`.
///
/// Spec §8: global stem-slug match across all docs; ties broken by minimum
/// directory distance to the source, then by lexicographically smallest
/// `id_path`. An optional path prefix `[[dir/sub/Name]]` (anchored at the
/// vault root, slugified componentwise) restricts the candidate set to docs
/// whose parent directory matches that prefix exactly.
pub fn resolve_in_ast<'a>(
    root: &'a AstNode<'a>,
    source: &Doc,
    stem_index: &HashMap<String, Vec<DocMeta>>,
) -> Vec<PathBuf> {
    let mut links: Vec<PathBuf> = Vec::new();

    // Collect first, then mutate: detaching a node's children mid-traversal
    // would disturb the `descendants()` iterator.
    let wikilinks: Vec<&'a AstNode<'a>> = root
        .descendants()
        .filter(|node| matches!(node.data.borrow().value, NodeValue::WikiLink(_)))
        .collect();

    for node in wikilinks {
        let target = match &node.data.borrow().value {
            NodeValue::WikiLink(w) => w.url.clone(),
            _ => continue,
        };
        let display = node_text(node);

        let replacement = match resolve(&target, &source.id_path, stem_index) {
            Some(doc) => {
                let url = permalink::to_url(&doc.output_path);
                if !links.contains(&doc.id_path) {
                    links.push(doc.id_path.clone());
                }
                render_link(&url, &display)
            }
            None => {
                eprintln!(
                    "warning: unresolved wikilink [[{}]] in {}",
                    target,
                    source.id_path.display()
                );
                render_nolink(&display)
            }
        };

        // Collapse the node into a single raw-HTML inline, discarding the
        // parsed label children (their text is already baked into `display`).
        for child in node.children().collect::<Vec<_>>() {
            child.detach();
        }
        node.data.borrow_mut().value = NodeValue::HtmlInline(replacement);
    }

    links
}

/// Concatenate the text of a node's descendant `Text` nodes — the wikilink's
/// display label. For the common plain-text label this is exactly the authored
/// text; any inline markup or raw HTML in a label degrades to its text content
/// (and raw HTML is dropped), which keeps the rendered anchor safe.
fn node_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut s = String::new();
    for child in node.descendants().skip(1) {
        if let NodeValue::Text(ref t) = child.data.borrow().value {
            s.push_str(t);
        }
    }
    s
}

fn render_link(url: &str, display: &str) -> String {
    format!(
        r#"<a class="wikilink" href="{}">{}</a>"#,
        html::escape(url),
        html::escape(display)
    )
}

fn render_nolink(display: &str) -> String {
    format!(r#"<span class="nolink">{}</span>"#, html::escape(display))
}

/// Split a wikilink target into an optional path prefix and a stem segment.
/// `"reference/Glossary"` → `(Some("reference"), "Glossary")`,
/// `"Hello"`              → `(None, "Hello")`.
/// A leading `/` survives in the prefix as the empty string, which causes
/// `prefix_matches` to require a root-level candidate.
fn split_prefix_stem(target: &str) -> (Option<&str>, &str) {
    match target.rfind('/') {
        Some(idx) => (Some(&target[..idx]), &target[idx + 1..]),
        None => (None, target),
    }
}

/// Edge count between two directory paths in the tree: drop the longest
/// shared component prefix, sum the lengths of what remains on each side.
/// `("blog/2025", "reference")` → 3 (up 2, down 1).
fn dir_distance(a: &Path, b: &Path) -> usize {
    let ac: Vec<Component> = a.components().collect();
    let bc: Vec<Component> = b.components().collect();
    let common = ac.iter().zip(bc.iter()).take_while(|(x, y)| x == y).count();
    (ac.len() - common) + (bc.len() - common)
}

/// True iff `parent`'s normalized components — slugified individually —
/// equal `prefix`'s slash-separated components, also slugified. Empty
/// segments (e.g. from a leading `/`) are ignored on the prefix side.
fn prefix_matches(parent: &Path, prefix: &str) -> bool {
    let prefix_slugs: Vec<String> = prefix
        .split('/')
        .filter(|s| !s.is_empty())
        .map(slug::slugify)
        .collect();
    let parent_slugs: Vec<String> = parent
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str().map(slug::slugify),
            _ => None,
        })
        .collect();
    parent_slugs == prefix_slugs
}

/// Resolve a wikilink target to a doc, per spec §8.
///
/// Slugify the stem (and prefix components, if a `dir/Name` form is used)
/// and scan the full doc set. Among candidates whose stem-slug matches —
/// and, if a prefix is given, whose parent dir also matches — pick the one
/// with the smallest directory distance from the source. Ties are broken
/// by the lexicographically smallest `id_path` so output is deterministic
/// across runs and platforms.
fn resolve<'a>(
    target: &str,
    source_id_path: &Path,
    stem_index: &'a HashMap<String, Vec<DocMeta>>,
) -> Option<&'a DocMeta> {
    let (prefix, stem) = split_prefix_stem(target);
    let stem_slug = slug::slugify(stem);
    if stem_slug.is_empty() {
        return None;
    }
    let empty = Path::new("");
    let source_dir = source_id_path.parent().unwrap_or(empty);

    // Candidates are pre-grouped by slugified stem, so the old per-doc stem
    // re-slugify-and-compare is already done — only the prefix/distance/lexical
    // tiebreak remains, over just the matching group.
    let candidates = stem_index.get(&stem_slug)?;
    let mut best: Option<(&DocMeta, usize)> = None;
    for doc in candidates {
        let cand_dir = doc.id_path.parent().unwrap_or(empty);
        if let Some(p) = prefix
            && !prefix_matches(cand_dir, p)
        {
            continue;
        }
        let dist = dir_distance(source_dir, cand_dir);
        best = match best {
            None => Some((doc, dist)),
            Some((curr, curr_dist)) => {
                let better =
                    dist < curr_dist || (dist == curr_dist && doc.id_path < curr.id_path);
                if better {
                    Some((doc, dist))
                } else {
                    Some((curr, curr_dist))
                }
            }
        };
    }
    best.map(|(doc, _)| doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_doc(id_path: &str) -> Doc {
        Doc {
            id_path: PathBuf::from(id_path),
            output_path: PathBuf::from(id_path).with_extension("html"),
            ..Default::default()
        }
    }

    fn doc_at(id_path: &str) -> DocMeta {
        DocMeta::from(&source_doc(id_path))
    }

    fn doc_with_permalink(id_path: &str, output_path: &str) -> DocMeta {
        let d = Doc {
            id_path: PathBuf::from(id_path),
            output_path: PathBuf::from(output_path),
            ..Default::default()
        };
        DocMeta::from(&d)
    }

    /// Build the stem index from `docs` and resolve `target`, returning the
    /// winning candidate's `id_path` (the only field these tests assert on).
    fn resolve_id(target: &str, source_id: &Path, docs: &[DocMeta]) -> Option<PathBuf> {
        let idx = build_stem_index(docs);
        resolve(target, source_id, &idx).map(|d| d.id_path.clone())
    }

    /// Parse `body` as Markdown with the wikilink extension, resolve wikilinks
    /// on the AST, and render to HTML — the same path `markup::render` drives.
    fn render_md(body: &str, source: &Doc, docs: &[DocMeta]) -> (String, Vec<PathBuf>) {
        let arena = comrak::Arena::new();
        let mut options = comrak::Options::default();
        options.render.r#unsafe = true;
        options.extension.wikilinks_title_after_pipe = true;
        let root = comrak::parse_document(&arena, body, &options);
        let stem_index = build_stem_index(docs);
        let links = resolve_in_ast(root, source, &stem_index);
        let mut out = String::new();
        comrak::format_html(root, &options, &mut out).unwrap();
        (out, links)
    }

    #[test]
    fn resolves_same_dir() {
        let source = source_doc("blog/a.md");
        let docs = vec![DocMeta::from(&source), doc_at("blog/b.md")];
        let (out, links) = render_md("see [[b]]", &source, &docs);
        assert!(out.contains(r#"<a class="wikilink" href="/blog/b.html">b</a>"#));
        assert_eq!(links, vec![PathBuf::from("blog/b.md")]);
    }

    #[test]
    fn walks_to_parent() {
        let source = source_doc("blog/2025/deep.md");
        let docs = vec![DocMeta::from(&source), doc_at("hello.md")];
        let (out, links) = render_md("see [[hello]]", &source, &docs);
        assert!(out.contains(r#"href="/hello.html""#));
        assert_eq!(links, vec![PathBuf::from("hello.md")]);
    }

    #[test]
    fn walks_multiple_levels() {
        let source = source_doc("a/b/c/d.md");
        let docs = vec![DocMeta::from(&source), doc_at("top.md")];
        let (out, _) = render_md("[[top]]", &source, &docs);
        assert!(out.contains(r#"href="/top.html""#));
    }

    #[test]
    fn uses_display_text() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (out, _) = render_md("[[b|click here]]", &source, &docs);
        assert!(out.contains(r#"<a class="wikilink" href="/b.html">click here</a>"#));
    }

    #[test]
    fn unresolved_emits_nolink_span() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source)];
        let (out, links) = render_md("[[Missing]]", &source, &docs);
        assert!(out.contains(r#"<span class="nolink">Missing</span>"#));
        assert!(links.is_empty());
    }

    #[test]
    fn records_links() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md"), doc_at("c.md")];
        let (_, links) = render_md("[[b]] and [[c]]", &source, &docs);
        assert_eq!(links, vec![PathBuf::from("b.md"), PathBuf::from("c.md")]);
    }

    #[test]
    fn dedups_links() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (_, links) = render_md("[[b]] [[b]] [[b|again]]", &source, &docs);
        assert_eq!(links, vec![PathBuf::from("b.md")]);
    }

    #[test]
    fn display_does_not_emit_raw_html() {
        // A label containing raw HTML must not produce executable markup.
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (out, _) = render_md("[[b|<script>alert(1)</script>]]", &source, &docs);
        assert!(!out.contains("<script>"));
    }

    #[test]
    fn wikilink_inside_code_fence_is_not_linked() {
        // The headline win of parsing-then-resolving: comrak never produces a
        // WikiLink node inside a code block, so `[[b]]` stays literal.
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (out, links) = render_md("```\n[[b]]\n```", &source, &docs);
        assert!(!out.contains("wikilink"));
        assert!(out.contains("[[b]]"));
        assert!(links.is_empty());
    }

    #[test]
    fn wikilink_inside_inline_code_is_not_linked() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (out, links) = render_md("use `[[b]]` syntax", &source, &docs);
        assert!(!out.contains("wikilink"));
        assert!(links.is_empty());
    }

    #[test]
    fn uses_to_url_for_index_html_dirs() {
        // A doc with a permalink-style output_path (trailing /index.html) should
        // render as a dir URL via to_url.
        let source = source_doc("a.md");
        let docs = vec![
            DocMeta::from(&source),
            doc_with_permalink("b.md", "blog/b/index.html"),
        ];
        let (out, _) = render_md("[[b]]", &source, &docs);
        assert!(out.contains(r#"href="/blog/b/""#));
    }

    #[test]
    fn resolve_first_match_wins_closer_dir() {
        // Two docs share stem `hello`; the one in the same dir as source should win
        // over the one in a parent dir.
        let source = doc_at("blog/post.md");
        let docs = vec![
            doc_at("hello.md"),      // parent-dir candidate
            doc_at("blog/hello.md"), // same-dir candidate (should win)
        ];
        let hit = resolve_id("hello", &source.id_path, &docs).unwrap();
        assert_eq!(hit, PathBuf::from("blog/hello.md"));
    }

    #[test]
    fn resolve_slugifies_target() {
        let source = doc_at("a.md");
        let docs = vec![doc_at("hello-world.md")];
        let hit = resolve_id("Hello World", &source.id_path, &docs).unwrap();
        assert_eq!(hit, PathBuf::from("hello-world.md"));
    }

    #[test]
    fn resolve_returns_none_when_no_match() {
        let source = doc_at("a.md");
        let docs = vec![doc_at("other.md")];
        assert!(resolve_id("missing", &source.id_path, &docs).is_none());
    }

    #[test]
    fn resolve_finds_cross_subtree_neighbor() {
        // Source's ancestor chain has no match, but a deep neighbor in a sibling
        // subtree does — the global lookup must find it.
        let source = doc_at("blog/2025/post.md");
        let docs = vec![doc_at("reference/glossary.md")];
        let hit = resolve_id("Glossary", &source.id_path, &docs).unwrap();
        assert_eq!(hit, PathBuf::from("reference/glossary.md"));
    }

    #[test]
    fn resolve_picks_nearest_when_ambiguous() {
        // Same stem in two subtrees; the one closer to source by directory
        // distance wins. blog/glossary.md (dist 1) beats reference/glossary.md
        // (dist 3) from blog/2025/post.md.
        let source = doc_at("blog/2025/post.md");
        let docs = vec![doc_at("reference/glossary.md"), doc_at("blog/glossary.md")];
        let hit = resolve_id("Glossary", &source.id_path, &docs).unwrap();
        assert_eq!(hit, PathBuf::from("blog/glossary.md"));
    }

    #[test]
    fn resolve_lexicographic_tiebreaker_on_equal_distance() {
        // Two candidates equidistant from source — pick the lexicographically
        // smaller id_path for determinism.
        let source = doc_at("root.md");
        let docs = vec![doc_at("zeta/glossary.md"), doc_at("alpha/glossary.md")];
        let hit = resolve_id("Glossary", &source.id_path, &docs).unwrap();
        assert_eq!(hit, PathBuf::from("alpha/glossary.md"));
    }

    #[test]
    fn resolve_explicit_prefix_picks_prefix_match_over_closer() {
        // Even though blog/glossary.md is closer, [[reference/Glossary]] must
        // resolve only to the prefix-matching candidate.
        let source = doc_at("blog/2025/post.md");
        let docs = vec![doc_at("blog/glossary.md"), doc_at("reference/glossary.md")];
        let hit = resolve_id("reference/Glossary", &source.id_path, &docs).unwrap();
        assert_eq!(hit, PathBuf::from("reference/glossary.md"));
    }

    #[test]
    fn resolve_explicit_prefix_no_fallback_to_bare_stem() {
        // [[missing/Hello]] must NOT fall back to a bare-stem `hello.md` at
        // the root — explicit prefixes are absolute.
        let source = doc_at("a.md");
        let docs = vec![doc_at("hello.md")];
        assert!(resolve_id("missing/Hello", &source.id_path, &docs).is_none());
    }

    #[test]
    fn resolve_slugifies_prefix_components() {
        // Prefix `Blog Posts` slugifies to `blog-posts`, matching the
        // candidate directory of the same slug.
        let source = doc_at("a.md");
        let docs = vec![doc_at("blog-posts/hello.md")];
        let hit = resolve_id("Blog Posts/Hello", &source.id_path, &docs).unwrap();
        assert_eq!(hit, PathBuf::from("blog-posts/hello.md"));
    }

    #[test]
    fn resolve_root_anchored_prefix() {
        // `[[/Hello]]` (leading slash → empty prefix) matches only a
        // root-level doc, not a nested one.
        let source = doc_at("blog/post.md");
        let docs = vec![doc_at("blog/hello.md"), doc_at("hello.md")];
        let hit = resolve_id("/Hello", &source.id_path, &docs).unwrap();
        assert_eq!(hit, PathBuf::from("hello.md"));
    }

    #[test]
    fn dir_distance_basics() {
        assert_eq!(dir_distance(Path::new("blog/2025"), Path::new("blog/2025")), 0);
        assert_eq!(dir_distance(Path::new("blog/2025"), Path::new("blog")), 1);
        assert_eq!(dir_distance(Path::new("blog/2025"), Path::new("")), 2);
        assert_eq!(dir_distance(Path::new("blog/2025"), Path::new("reference")), 3);
        assert_eq!(dir_distance(Path::new("a/b/c"), Path::new("a/x/y")), 4);
    }
}
