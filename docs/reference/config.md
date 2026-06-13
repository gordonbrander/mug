# Configuration reference

Site-wide configuration lives in `config.yaml` at the project root. **Every key
is optional** — italic builds with no config file at all (and a config file
containing only comments is treated the same as a missing one).

A complete `config.yaml` with every key at its default:

```yaml
# Directories (relative to the project root)
content_dir: content
output_dir: public
templates_dir: templates
static_dir: static
data_dir: data
archives_dir: archives

# Optional theme; no default.
# theme: themes/my-theme

# Extract inline `#hashtags` into the `tags` taxonomy. Off by default.
hashtags: false

site: {}          # site metadata, reachable as {{ site.* }} in templates
collections: {}   # named queries
taxonomies: []    # declared taxonomy field names
defaults: {}      # per-collection default frontmatter
# related:        # weights for the related filter; defaults derived (see below)
#   weights: {}
```

## Directories

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `content_dir` | path | `content` | Source content (`.md`, `.html`, `.yaml`). |
| `output_dir` | path | `public` | Build output. Removed by `italic clean`. |
| `templates_dir` | path | `templates` | Tera layouts, partials, and macros. |
| `static_dir` | path | `static` | Copied verbatim into the output. |
| `data_dir` | path | `data` | YAML files surfaced as `{{ data.* }}`. |
| `archives_dir` | path | `archives` | Archive templates (see [Archives](../guides/archives.md)). |

All paths are relative to the working directory.

## `theme`

| Type | Default |
|------|---------|
| path | none |

Path to a theme folder. When set, the theme's `templates/`, `archives/`, and
`static/` are overlaid **beneath** your site's: your own files win per-path,
and anything you don't provide falls through to the theme. The theme's
`config.yaml` supplies defaults that your config overrides. Themes don't nest —
a `theme:` key inside a theme's own config is ignored. Full layering rules in
the [Themes guide](../guides/themes.md).

```yaml
theme: themes/obsidian
```

## `hashtags`

| Type | Default |
|------|---------|
| bool | `false` |

When `true`, the markup phase scans Markdown bodies for inline `#hashtags`,
adds them to each doc's `tags` taxonomy, and strips them from the rendered
HTML. Off by default so a literal `#` in prose is untouched. A theme may enable
it; either the theme or the site setting it turns it on.

## `site`

A free-form map of site metadata. Everything under `site:` is reachable in
templates as `{{ site.<key> }}`. Two keys also have built-in meaning:

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `site.url` | string | none | Origin for absolute URLs, e.g. `https://example.com`. A trailing slash is trimmed. When unset, filters that produce absolute URLs degrade gracefully to root-relative ones. |
| `site.base_path` | string | `""` | Subpath the site is hosted under, e.g. `/blog`. Normalized to start with `/` and not end with one (`blog/`, `/blog/`, and `blog` all become `/blog`). |

```yaml
site:
  title: My Site
  description: A site built with italic.
  url: https://example.com
  base_path: ""
```

## `collections`

A map of name → query. Each collection is a saved query over your content,
evaluated once per build and readable in templates via
`collection(name="...")`. Order in `config.yaml` is preserved.

```yaml
collections:
  posts:
    path: "posts/*.md"
    order_by: date
    sort: desc
```

Query keys:

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `path` | glob | none | Glob pattern matched against paths in `content_dir`. |
| `order_by` | `title` \| `date` \| `updated` | `date` | Sort field. |
| `sort` | `asc` \| `desc` | `desc` | Sort direction. |
| `omit` | array of `id_path` | `[]` | Specific documents to exclude. |

Unknown query keys are an error (typos fail loudly). There is no `limit` key —
capping is a render-time concern; pass `limit=` to the `collection()` function
or set `limit:` on an archive.

### The `all` collection

A collection named `all` always exists. If you don't declare one, the build
injects it with the default query (every doc, date descending). It backs the
`all()` function and is also readable as `collection(name="all")` — handy for a
`sitemap.xml` or full archive. Declare your own `all` under `collections:` to
change its order or contents (a `path`/`omit` may narrow it below every doc); a
site `all` overrides a theme's, like any other collection.

## `taxonomies`

An array of frontmatter field names to treat as taxonomies. There are no
built-in defaults — declare `tags` like any other taxonomy. Declaration order
is preserved.

```yaml
taxonomies:
  - tags
  - category
  - series
```

See the [Taxonomies guide](../guides/taxonomies.md).

## `defaults`

A map of collection name → default frontmatter. Values fill in keys that
members of that collection did not set themselves; a document's own frontmatter
always wins. Every entry must name a declared collection (a theme's collection
counts) — an unknown name is an error.

```yaml
defaults:
  posts:
    permalink: /blog/:yyyy/:mm/:dd/:slug/
    template: post.html
```

When a document belongs to multiple collections with overlapping defaults, the
later entry (in config order) wins.

## `related`

Weights for the [`related`](templates.md#related--pages-related-to-this-page)
filter. `weights` is the only allowed key — `limit` and `omit` are filter
arguments, not config, and an unknown or stale key is an error with a pointer
to the replacement.

```yaml
related:
  weights:
    tags: 2.0    # any declared taxonomy
    links: 1.0   # the wikilink graph (links, backlinks, co-citations)
```

When the block (or its `weights`) is absent, italic fills in equal weight `1.0`
on every declared taxonomy plus the `links` graph, so related pages work
zero-config. When a theme sets weights and your site doesn't, the theme's are
inherited; if your site sets any weights, they win wholesale.

See the [Related pages guide](../guides/related.md).

## Theme config merging

When `theme:` is set, the theme's own `config.yaml` is loaded and layered
beneath yours:

- `collections` and `defaults` merge **by name** — the theme's entries come
  first, your same-named entry replaces the theme's in place, your new entries
  append.
- `taxonomies` are unioned in order (theme first, then yours, deduplicated).
- `site:` is deep-merged, your values winning per key.
- `hashtags` is on if either side enables it.
- `related.weights`: yours win wholesale if set, otherwise the theme's.
- The theme's `*_dir` keys are ignored — a theme always uses its conventional
  `templates/`, `archives/`, `static/` subdirs.
- `content_dir`, `output_dir`, and `data_dir` are always yours; a theme never
  ships content or data.

## See also

- [Themes guide](../guides/themes.md)
- [Collections guide](../guides/collections.md)
- [Frontmatter reference](frontmatter.md)
