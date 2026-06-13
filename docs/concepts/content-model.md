# Content model

Every file in `content/` with a recognized extension becomes a **document** — 
the unit everything else in italic operates on. Collections query documents,
taxonomies group them, templates render them, archives list them.

## Three content types

| Type | Frontmatter | Body |
|------|-------------|------|
| `.md` | Optional YAML block | Markdown → rendered to HTML |
| `.html` | Optional YAML block | Raw HTML → passed through |
| `.yaml` | The whole file | `content:` field rendered as HTML |

`.md` and `.html` use the conventional `---`-delimited frontmatter block. A
`.yaml` document is *all* frontmatter — useful for data-heavy pages — with the
optional `content:` string field as its body.

## `id_path`: a document's identity

A document's **`id_path`** is its path relative to `content/`, e.g.
`posts/hello.md`. It is the canonical identity used everywhere:

- `doc(id_path="about.md")` looks a document up by it.
- `omit=[page.id_path]` excludes documents from listings by it.
- The URL filters (`| link`, `| permalink`) resolve it to the document's
  rendered location.
- Collection `path:` globs match against it.

Distinct from `id_path` is the **output path** — where the document renders.
By default the output path mirrors the `id_path` with an `.html` extension;
a [`permalink:`](../guides/permalinks.md) changes the output path but never the
`id_path`.

## Frontmatter becomes the page

When a document loads, its frontmatter is *uplifted* into typed fields:

- Special keys (`title`, `summary`, `draft`, `template`, `date`, `updated`,
  `permalink`, and declared taxonomy fields) get parsed, defaulted, and exposed
  as `page.title`, `page.date`, etc.
- Taxonomy fields turn into `page.terms` — a map of taxonomy → term slug →
  display text (`page.terms.tags`).
- **Everything else is preserved verbatim** on `page.data`, reachable as
  `{{ page.data.your_key }}`. (The special keys remain visible there too.)

Dates have a filesystem fallback: `date` defaults to the file's created time
(then modified time), `updated` to the modified time. Frontmatter always wins
when present. The full rules are in the
[frontmatter reference](../reference/frontmatter.md).

Defaults can also flow in from config: a collection's
[`defaults:`](../reference/config.md#defaults) entry fills any key its members
didn't set themselves.

## Documents vs. archive pages

Archive templates in `archives/` generate *view pages* — paginated listings,
feeds, sitemaps — from collections and taxonomies. View pages are rendered
output only: they are never classified back into collections, taxonomies, or
backlinks, so a tag page can't tag itself. See
[Archives](../guides/archives.md).

## See also

- [The build pipeline](build-pipeline.md) — how documents flow to output
- [Authoring guide](../guides/authoring.md)
