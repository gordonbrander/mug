use comrak::nodes::{AstNode, NodeValue};

/// Scan the `Text` nodes of a parsed comrak AST for inline `#hashtag`s, strip
/// each one from the rendered output, and return the raw tag texts (without the
/// leading `#`). The caller slugifies and de-dups them into `doc.tags` via
/// [`crate::doc::insert_tag`].
///
/// A tag is a `#` that sits at a word boundary — the start of a Text node or
/// right after whitespace, so `page#section` and `C#` are left alone — followed
/// by a run of `[A-Za-z0-9_\-/]` that contains at least one ASCII letter (so a
/// bare `#123` issue reference is ignored). The returned text is that run with
/// any leading/trailing `-`/`/` trimmed; the whole run (plus the `#`) is removed
/// from the node, collapsing one adjacent space so `a #foo b` becomes `a b`.
///
/// Scanning the parsed AST rather than the raw source is what makes this safe:
/// comrak turns heading markers (`# Heading`) and code spans/fences into their
/// own node types, so the `#` in those contexts never reaches a `Text` node and
/// can't be mistaken for a tag — mirroring why the wikilink pass moved onto the
/// AST.
pub fn extract_in_ast<'a>(root: &'a AstNode<'a>) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();

    // Collect first, then mutate — rewriting a node's value while iterating
    // `descendants()` is the same hazard the wikilink pass guards against.
    let text_nodes: Vec<&'a AstNode<'a>> = root
        .descendants()
        .filter(|node| matches!(node.data.borrow().value, NodeValue::Text(_)))
        .collect();

    for node in text_nodes {
        let original = match &node.data.borrow().value {
            NodeValue::Text(t) => t.clone(),
            _ => continue,
        };
        let (stripped, found) = scan(&original);
        if found.is_empty() {
            continue;
        }
        tags.extend(found);
        node.data.borrow_mut().value = NodeValue::Text(stripped.into());
    }

    tags
}

fn is_tag_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/'
}

/// Walk `text`, pulling out hashtag tokens and returning the body with them
/// removed alongside the list of tag texts (leading `#` stripped, surrounding
/// `-`/`/` trimmed).
fn scan(text: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut tags: Vec<String> = Vec::new();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        let at_boundary = i == 0 || chars[i - 1].is_whitespace();
        if c == '#' && at_boundary {
            let mut j = i + 1;
            while j < len && is_tag_char(chars[j]) {
                j += 1;
            }
            let token: String = chars[i + 1..j].iter().collect();
            let trimmed = token.trim_matches(|c| c == '-' || c == '/');
            if j > i + 1 && trimmed.chars().any(|c| c.is_ascii_alphabetic()) {
                tags.push(trimmed.to_string());
                // Drop the `#…` run, then swallow one following space when the
                // output already ends at a boundary, so the gap the tag leaves
                // collapses instead of doubling. Trailing space before the next
                // inline node is preserved (don't fuse `see ` + emphasis).
                let mut k = j;
                if (out.is_empty() || out.ends_with(char::is_whitespace))
                    && k < len
                    && chars[k].is_whitespace()
                {
                    k += 1;
                }
                i = k;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }

    (out, tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse `body` as Markdown, run the hashtag pass, and render to HTML —
    /// the same parse→extract→render path `markup::render` drives.
    fn run(body: &str) -> (String, Vec<String>) {
        let arena = comrak::Arena::new();
        let options = comrak::Options::default();
        let root = comrak::parse_document(&arena, body, &options);
        let tags = extract_in_ast(root);
        let mut out = String::new();
        comrak::format_html(root, &options, &mut out).unwrap();
        (out, tags)
    }

    #[test]
    fn extracts_and_strips_simple_tag() {
        let (html, tags) = run("Loving #rust here");
        assert_eq!(tags, vec!["rust"]);
        assert!(!html.contains("#rust"));
        assert!(html.contains("Loving here"));
    }

    #[test]
    fn allows_slug_paths() {
        let (_, tags) = run("see #project/mug");
        assert_eq!(tags, vec!["project/mug"]);
    }

    #[test]
    fn requires_word_boundary() {
        let (html, tags) = run("see page#section for more");
        assert!(tags.is_empty());
        assert!(html.contains("page#section"));
    }

    #[test]
    fn ignores_pure_numeric() {
        let (html, tags) = run("fixed in #123 today");
        assert!(tags.is_empty());
        assert!(html.contains("#123"));
    }

    #[test]
    fn code_fence_stays_literal() {
        let (html, tags) = run("```\n#nope\n```");
        assert!(tags.is_empty());
        assert!(html.contains("#nope"));
    }

    #[test]
    fn inline_code_stays_literal() {
        let (html, tags) = run("use `#nope` here");
        assert!(tags.is_empty());
        assert!(html.contains("#nope"));
    }

    #[test]
    fn heading_marker_is_not_a_tag() {
        let (html, tags) = run("# Heading\n\nbody");
        assert!(tags.is_empty());
        assert!(html.contains("<h1>Heading</h1>"));
    }

    #[test]
    fn multiple_tags_in_one_node() {
        let (html, tags) = run("tagged #alpha and #beta done");
        assert_eq!(tags, vec!["alpha", "beta"]);
        assert!(html.contains("tagged and done"));
    }

    #[test]
    fn collapses_surrounding_whitespace() {
        let (out, tags) = scan("a #foo b");
        assert_eq!(tags, vec!["foo"]);
        assert_eq!(out, "a b");
    }

    #[test]
    fn leading_tag_drops_following_space() {
        let (out, tags) = scan("#foo bar");
        assert_eq!(tags, vec!["foo"]);
        assert_eq!(out, "bar");
    }

    #[test]
    fn trailing_punctuation_stays() {
        let (out, tags) = scan("end with #foo.");
        assert_eq!(tags, vec!["foo"]);
        assert_eq!(out, "end with .");
    }
}
