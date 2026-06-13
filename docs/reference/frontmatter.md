# Frontmatter reference

Frontmatter is a YAML block at the top of a document, delimited by `---` lines:

```markdown
---
title: Hello, world
template: base.html
date: 2026-01-01
tags: [intro]
---
The body of the post goes here.
```

`.md` and `.html` files take an optional frontmatter block; in a `.yaml` file
the whole file is frontmatter and the `content:` field (if present) is the
body. Missing or unterminated frontmatter is treated as no frontmatter;
malformed YAML inside a present block is a build error.

## Document keys

A few keys have special meaning and sensible defaults when absent:

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `title` | string | `""` | Document title, `{{ page.title }}`. |
| `summary` | string | `""` | Brief description, `{{ page.summary }}`. |
| `draft` | bool | `false` | Exclude from builds (see [Drafts](../guides/drafts.md)). |
| `template` | string | none | Template to wrap the body in. Without one, the rendered body is the final output. |
| `date` | date | file created time, falling back to modified time | Publication date. |
| `updated` | date | file modified time | Last-modified date. |
| `permalink` | string | mirror of the source path with `.html` | Output location pattern (see below). |
| `aliases` | array of strings | `[]` | Old URLs that should redirect here (see [Aliases](../guides/aliases.md)). |
| `<taxonomy>` | array of strings | `[]` | One field per declared taxonomy, e.g. `tags: [intro, rust]`. |
| `content` | string | `""` | `.yaml` files only: the body to render. |

Dates parse as RFC 3339 (`2026-01-01T12:00:00Z`) or plain `YYYY-MM-DD`.
Frontmatter dates win; the filesystem only fills in when the frontmatter value
is absent or unparseable.

**Any other key is preserved verbatim** and reachable in templates as
`{{ page.data.<key> }}`. (The special keys above are also still present in
`page.data`.) A document's taxonomy memberships are uplifted into
`page.terms` — a map of taxonomy → term slug → display text, e.g.
`page.terms.tags`.

## Permalink patterns

`permalink:` overrides the default output location (which mirrors the source
path: `notes/foo.md` → `notes/foo.html`).

| Variable | Expands to |
|----------|------------|
| `:slug` | Slugified stem of the source filename. |
| `:yyyy` | Four-digit year of `date`. |
| `:mm` | Two-digit month of `date`. |
| `:dd` | Two-digit day of `date`. |
| `:term` | Term slug — taxonomy archives only; left untouched elsewhere. |

A leading `/` is ignored; a trailing `/` writes `index.html` in that directory:

```yaml
permalink: /blog/:yyyy/:slug/   # → blog/2026/hello/index.html
```

See the [Permalinks guide](../guides/permalinks.md).

## Aliases

`aliases:` lists old URLs that should redirect to this document — useful after
renaming or reorganizing a published note:

```yaml
aliases:
  - /old-url/
  - /posts/legacy.html
```

Each entry is a literal historical URL (no pattern expansion) and emits a small
redirect page at that path. A trailing slash or extension-less entry writes
`index.html` in that directory; an entry with an extension is written as that
literal file. See the [Aliases guide](../guides/aliases.md).

## Setting defaults per collection

Rather than repeating frontmatter on every file, set collection-wide defaults
under `defaults:` in `config.yaml`. A document's own frontmatter always
overrides a default. See [`defaults`](config.md#defaults).

## Archive keys

Templates in `archives/` use their own frontmatter vocabulary (see the
[Archives guide](../guides/archives.md)):

| Key | Type | Required | Meaning |
|-----|------|----------|---------|
| `kind` | `collection` \| `taxonomy` | yes | What the archive iterates over. |
| `collection` | string | with `kind: collection` | Name of the collection. |
| `taxonomy` | string | with `kind: taxonomy` | Name of the taxonomy; emits one page-set per term. |
| `permalink` | string | yes | Output pattern; `:term` available for taxonomy archives. Pages ≥ 2 get `page/N/` appended automatically. |
| `per_page` | integer | no | Items per page. Without it, everything lands on one page. |
| `limit` | integer | no | Cap on items **before** pagination. For a collection archive it caps the total; for a taxonomy archive it caps items per term. |
| `query` | mapping | no | **Taxonomy archives only.** Scopes and re-orders each term's docs before pagination, using the same `path` / `order_by` / `sort` / `omit` keys as a collection [query](config.md#collections). An error on `kind: collection` archives. |
| `template` | string | no | Layout to wrap each rendered archive page. |

## See also

- [Authoring guide](../guides/authoring.md)
- [Configuration reference](config.md)
- [Template reference](templates.md) — how `page.*` is consumed
