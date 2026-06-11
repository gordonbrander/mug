# Collections

Collections are saved queries over your content. They're how italic turns a
free-form folder of files into blogs, sections, and custom groupings — without
dictating where files live.

## Defining a collection

Declare collections in `config.yaml`:

```yaml
collections:
  posts:
    path: "posts/*.md"
    order_by: date
    sort: desc
```

That's a blog: a reverse-chronological collection of everything matching
`posts/*.md`. Define as many as you want — multiple blogs, a news feed, and a
portfolio can coexist in one site, because collections are queries, not
directories.

Query keys:

| Key | Default | Meaning |
|-----|---------|---------|
| `path` | — | Glob matched against paths in `content/`. |
| `order_by` | `date` | `title` \| `date` \| `updated`. |
| `sort` | `desc` | `asc` \| `desc`. |
| `omit` | `[]` | Specific documents to exclude, by `id_path`. |

Unknown keys are a build error, so typos fail loudly. There's deliberately no
`limit` key — capping is a render-time concern (see below).

## Using collections in templates

Read a collection with the `collection()` function:

```jinja
{% for post in collection(name="posts", limit=10) %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

`omit` and `limit` are render-time arguments: `omit` layers on top of the
collection's definition-time `omit`, and is applied before `limit`. Handy when
a page wants to exclude itself from a collection it belongs to:

```jinja
{% for post in collection(name="posts", omit=[page.id_path], limit=5) %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

## Defaults

Rather than repeating the same frontmatter in every member file, set defaults
for the collection:

```yaml
collections:
  posts:
    path: "posts/*.md"

defaults:
  posts:
    permalink: /blog/:yyyy/:mm/:dd/:slug/
    template: post.html
```

Every member of `posts` now gets a dated permalink and the `post.html` layout
without writing either in its frontmatter. Precedence:

1. A document's own frontmatter always wins.
2. When a document belongs to multiple collections with overlapping defaults,
   the last matching `defaults:` entry (in config order) wins.

A `defaults:` entry must name a declared collection (a theme's counts), or the
build fails.

## Archives from collections

Generate a paginated listing page (or RSS feed) for a collection with an
archive template — see [Archives, feeds & sitemaps](archives.md).

## See also

- [Configuration reference: collections](../reference/config.md#collections)
- [Template reference: collection()](../reference/templates.md#collectionname--list-a-named-collection)
- [Permalinks](permalinks.md)
