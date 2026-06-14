# Archives, feeds & sitemaps

An **archive** is a template in `archives/` that generates output pages from a
collection or taxonomy: paginated blog listings, tag pages, RSS feeds,
sitemaps. One archive template can emit one page, a paginated series, or a
whole family of pages (one per tag).

## Collection archives

`kind: collection` paginates a named collection. Example `archives/blog.html`:

```yaml
---
kind: collection
collection: posts
permalink: /blog/
per_page: 10
template: blog-archive.html
---
{% for post in pagination.items %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

The body renders once per page with a `pagination` context. Page 1 lands at
the `permalink`; pages ≥ 2 get `page/N/` appended automatically
(`/blog/` → `/blog/page/2/`).

## Taxonomy archives

`kind: taxonomy` emits one (optionally paginated) page-set per term. `:term`
in the permalink is the term's slug, and the body receives a `term` object
(`slug`, `text`):

```yaml
---
kind: taxonomy
taxonomy: tags
permalink: /tags/:term/
---
<h1>{{ term.text }}</h1>
{% for post in pagination.items %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

## Scoping a taxonomy archive with `query:`

A taxonomy is global: a term aggregates docs from *every* path, so by default a
`tags` archive lists everything tagged `rust` regardless of where it lives. To
scope an archive to one section — say, a `/posts/tags/:term/` page-set that only
covers `posts/**` — add a `query:` sub-mapping. It takes the same
`path` / `order_by` / `sort` / `omit` keys as a collection
[query](../reference/config.md#collections), and is applied per term, just before
pagination:

```yaml
---
kind: taxonomy
taxonomy: tags
permalink: /posts/tags/:term/
query:
  path: "posts/**"   # only docs under posts/ count toward each term
  order_by: title
  sort: asc
---
<h1>{{ term.text }}</h1>
{% for post in pagination.items %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

A term whose docs are *all* filtered out emits no page, so a tag used only outside
`posts/**` simply doesn't appear. `query:` is taxonomy-only — on a
`kind: collection` archive it's an error, since a collection archive already draws
from its named collection's query; define the filtered collection in `config.yaml`
instead. The render-phase counterpart for one-off scoping inside a template is the
[`filter_by_id_path`](../reference/templates.md#filter_by_id_path--keep-docs-matching-a-path-glob)
filter.

## Capping with `limit`

`limit:` caps how many items an archive covers — useful since an archive
references a collection by name and can't pass render-time arguments:

```yaml
---
kind: collection
collection: posts
permalink: /blog/
limit: 100      # paginate at most the first 100 items…
per_page: 10    # …10 per page → 10 pages
---
```

`limit` and `per_page` compose: `limit` caps the item set, then `per_page`
splits the capped set into pages. For a collection archive `limit` caps the
total; for a taxonomy archive it caps items *per term*. "First N" follows the
collection's query order, or date-descending for a taxonomy.

## The `pagination` context

| Field | Meaning |
|-------|---------|
| `pagination.items` | The docs on this page. |
| `pagination.current` | Current page number (1-indexed). |
| `pagination.total` | Total number of pages. |
| `pagination.prev_url` | Previous page's URL; unset on the first page. |
| `pagination.next_url` | Next page's URL; unset on the last page. |

Because the URLs are unset (not empty) at the ends, prev/next navigation only
renders when there's somewhere to go:

```html
<nav class="pagination">
  {% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">← Previous</a>{% endif %}
  <span>Page {{ pagination.current }} of {{ pagination.total }}</span>
  {% if pagination.next_url %}<a href="{{ pagination.next_url }}">Next →</a>{% endif %}
</nav>
```

## Built-in feeds and sitemap

italic generates a sitemap and RSS feeds automatically — you don't have to write
any archive for them. By default it emits `/sitemap.xml` over every doc and
`/feed/all.xml`, configurable with the [`sitemap`](../reference/config.md#sitemap)
and [`feed`](../reference/config.md#feed) config keys (each can name other
collections or be disabled). To customize the markup, drop a disk archive at the
same path — `archives/sitemap.xml` or `archives/feed/<name>.xml` shadows the
built-in, and is an ordinary collection archive like the ones above.

## How archives fit the pipeline

Archives read only the classification of source content — never each other's
output — so they are order-independent and run in parallel. An archive page is
never itself classified into collections, taxonomies, or backlinks. See
[the build pipeline](../concepts/build-pipeline.md).

## See also

- [Frontmatter reference: archive keys](../reference/frontmatter.md#archive-keys)
- [Collections](collections.md) · [Taxonomies](taxonomies.md)
- [Permalinks](permalinks.md)
