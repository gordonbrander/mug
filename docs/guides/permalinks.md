# Permalinks

By default a document renders to a location mirroring its source path —
`notes/foo.md` becomes `notes/foo.html`. The `permalink` key overrides that,
per document or per collection.

## Setting a permalink

In frontmatter:

```yaml
---
permalink: /blog/:yyyy/:slug/
---
```

Or once for a whole collection in `config.yaml`:

```yaml
defaults:
  posts:
    permalink: /blog/:yyyy/:mm/:dd/:slug/
```

## Pattern variables

| Variable | Expands to |
|----------|------------|
| `:slug` | Slugified stem of the source filename. |
| `:yyyy` | Four-digit year of the document's `date`. |
| `:mm` | Two-digit month. |
| `:dd` | Two-digit day. |
| `:term` | Term slug — [taxonomy archives](archives.md#taxonomy-archives) only. |

Two path rules:

- A leading `/` is ignored (patterns are always relative to the output root).
- A trailing `/` writes `index.html` in that directory, giving you clean URLs:
  `/blog/:yyyy/:slug/` → `blog/2026/hello/index.html`, served as
  `/blog/2026/hello/`.

Paginated archive pages get `page/N/` appended to the landing permalink
automatically (`/blog/` → `/blog/page/2/`).

## URLs: site URL and base path

Two `site:` keys control how paths become full URLs:

```yaml
site:
  url: https://example.com   # origin for absolute URLs
  base_path: ""              # subpath the site is hosted under, e.g. "/blog"
```

And four template filters build URLs from paths:

| Filter | Input | Output |
|--------|-------|--------|
| `permalink` | `id_path` | Absolute URL (`site.url` + output path). |
| `link` | `id_path` | Root-relative URL. |
| `relative_url` | any path | `base_path` + `/` + path. |
| `absolute_url` | any path | `site.url` + `base_path` + `/` + path. |

Rule of thumb: `permalink`/`link` for linking to *documents* (they resolve
`id_path` to wherever the document actually renders), `relative_url`/
`absolute_url` for *assets* and hand-built paths. Feeds and social tags want
the absolute forms; in-site navigation is happy with the relative ones. When
`site.url` is unset, absolute filters degrade gracefully to root-relative.

Hosting under a subpath (e.g. GitHub project pages at
`username.github.io/repo/`)? Set `base_path: /repo` and use the URL filters
everywhere instead of hardcoding `/`-prefixed hrefs — see
[Deployment](deployment.md).

## See also

- [Frontmatter reference](../reference/frontmatter.md#permalink-patterns)
- [Template reference: URL filters](../reference/templates.md#url-filters)
