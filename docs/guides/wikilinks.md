# Wikilinks & backlinks

Wikilinks are the connective tissue of a digital garden: link notes by title,
and italic resolves the target, builds the backlink graph, and feeds the
[related-pages](related.md) engine.

## Syntax

```markdown
[[Page Title]]
[[Page Title|Display text]]
[[reference/Glossary]]          # path-prefixed form
```

- `[[Page Title]]` links to the page whose filename stem slugifies to
  `page-title`, displaying the authored text.
- `[[Page Title|Display text]]` links the same way but shows `Display text`.
- `[[dir/sub/Name]]` restricts matching to docs whose parent directory equals
  that prefix, anchored at the content root. A leading slash
  (`[[/Name]]`) requires a top-level document.

Wikilinks inside code spans and fenced code blocks stay literal — they are
resolved after Markdown parsing, so `` `[[not a link]]` `` renders as written.

## How targets resolve

Resolution mirrors Obsidian's behavior:

1. The target stem is slugified and matched against the slugified filename
   stems of **all** documents.
2. Among matches, the winner is the one with the smallest directory distance
   from the linking document — your own folder beats a sibling folder beats a
   distant one.
3. Remaining ties break by lexicographically smallest `id_path`, so builds are
   deterministic.

A resolved link renders as an anchor; an unresolved one as a span:

```html
<a class="wikilink" href="…">Display text</a>
<span class="nolink">Display text</span>
```

Style `.wikilink` and `.nolink` in your CSS to make link state visible —
gardens often render unresolved links in a muted color.

## Backlinks

Every resolved wikilink registers an edge in the site's link graph. Only
`[[wikilinks]]` count — a plain Markdown `[label](other.md)` link does not.
The wikilink syntax is the intentional "this is a cross-document reference"
signal, and backlinks reflect that.

Render a page's backlinks in its layout with the `backlinks` filter:

```jinja
<h2>Linked from</h2>
<ul>
{% for src in page.id_path | backlinks(order_by="title", sort="asc") %}
  <li><a href="{{ src.id_path | link }}">{{ src.title }}</a></li>
{% endfor %}
</ul>
```

Kwargs (`order_by`, `sort`, `omit`, `limit`) are in the
[template reference](../reference/templates.md#backlinks--pages-that-link-to-this-one).

The link graph is also one of the namespaces the [related-pages](related.md)
engine scores — including co-citations (two pages linking to the same third
page), which backlinks alone don't capture.

## See also

- [Related pages](related.md)
- [Template reference: backlinks](../reference/templates.md#backlinks--pages-that-link-to-this-one)
- [Tutorial: publish your Obsidian vault](../getting-started/tutorial.md)
