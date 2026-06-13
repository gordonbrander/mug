# Template reference

Italic templates use [Tera](https://keats.github.io/tera/docs/), a Jinja-style
template language. All of [Tera's built-in filters and
functions](https://keats.github.io/tera/docs/#built-ins) are available, plus
the italic-specific functions and filters documented here.

## Template files

Templates are any `.html`, `.xml`, `.tera`, `.json`, or `.txt` file under
`templates/`. Only `.html` and `.xml` are HTML-autoescaped; in `.tera`,
`.json`, and `.txt` templates characters like `&`, `<`, and `/` pass through
verbatim (which is what JSON and plain text want). Use `.json`/`.txt` to
template those formats directly (a JSON feed, a `robots.txt`) and `.tera` as a
generic escape hatch for any other format.

## The two phases

Tera runs twice during a build, with different powers:

- **Content phase** — each document's body is rendered as a Tera template
  *before* Markdown rendering. This enables macros and partials inside
  content. The page index does not exist yet, so functions that read other
  pages (`collection()`, `all()`, `taxonomy()`, `doc()`) and filters that read
  the link graph (`backlinks`, `related`) are unavailable.
- **Template phase** — layouts in `templates/` render each document and
  archive page. Everything is available.

Each entry below states where it works. "Both" means template and content
phase.

## Context

Inside a template the available variables are:

| Variable | Available | Contents |
|----------|-----------|----------|
| `page` | both phases | The current document — see below. |
| `site` | both phases | The `site:` submap from `config.yaml`. |
| `data` | both phases | Every top-level YAML file in `data/`, keyed by filename stem. |
| `pagination` | archive pages | Pagination state — see below. |
| `term` | taxonomy archive pages | `{slug, text}` of the current term. |

`page` fields:

| Field | Contents |
|-------|----------|
| `page.title` | Title from frontmatter (`""` if unset). |
| `page.summary` | Summary from frontmatter (`""` if unset). |
| `page.content` | The rendered body (template phase). Pipe through `safe`. |
| `page.date`, `page.updated` | Dates (frontmatter, falling back to file times). |
| `page.id_path` | The document's source path — its canonical identity, used by `doc()`, `omit=`, and the URL filters. |
| `page.terms` | Map of taxonomy → term slug → display text, e.g. `page.terms.tags`. |
| `page.data` | The full frontmatter map — any custom key, e.g. `page.data.blurb`. |

`pagination` fields (archive pages only):

| Field | Contents |
|-------|----------|
| `pagination.items` | The docs on this page. |
| `pagination.current` | Current page number (1-indexed). |
| `pagination.total` | Total number of pages. |
| `pagination.prev_url` | URL of the previous page; **unset** on the first page. |
| `pagination.next_url` | URL of the next page; **unset** on the last page. |

Because `prev_url`/`next_url` are unset (not empty) at the ends, guard them
with `{% if %}`:

```jinja
{% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">← Previous</a>{% endif %}
```

## Functions

### `collection(name=...)` — list a named collection

**Template phase.** Returns the members of a collection declared under
`collections:` in `config.yaml`, in the collection's configured order.

| Kwarg | Required | Meaning |
|-------|----------|---------|
| `name` | yes | Collection name. |
| `omit` | no | Array of `id_path` strings to exclude; layers on top of the collection's definition-time `omit`. |
| `limit` | no | Max items; applied after `omit`. |

```jinja
{% for post in collection(name="recent_posts", omit=[page.id_path], limit=5) %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

### `all()` — list every doc

**Template phase.** Returns the always-present `all` collection — every document
on the site, by default in date-descending (newest-first) order, with no
`config.yaml` setup. It is backed by a collection named `all` that the build
injects automatically; declaring `collections: { all: ... }` in `config.yaml`
lets you reorder, omit, or filter what `all()` (and `collection(name="all")`)
returns. Takes **no arguments** — passing any is an error rather than a silent
no-op. To order, limit, or filter ad hoc, redefine the `all` collection or pipe
through array filters (`omit_docs`, `dirtree`, Tera's `slice`).

```jinja
{% for doc in all() %}
  <a href="{{ doc.id_path | link }}">{{ doc.title }}</a>
{% endfor %}
```

### `taxonomy(name=...)` — list a taxonomy's terms

**Template phase.** Returns a map of term slug → docs for a declared taxonomy.
Iterate deterministically with [`entries`](#entries--iterate-a-map-in-key-order).

```jinja
{% for slug, docs in taxonomy(name="tags") %}
  <h2>{{ slug }}</h2>
  {% for post in docs %}<a href="{{ post.id_path | permalink }}">{{ post.title }}</a>{% endfor %}
{% endfor %}
```

### `doc(id_path=...)` — look up a single doc

**Template phase.** Fetch one document by `id_path`. Returns `null` for an
unknown path, so guard with `{% if %}` rather than failing the build:

```jinja
{% set about = doc(id_path="about.md") %}
{% if about %}<a href="{{ about.id_path | link }}">{{ about.title }}</a>{% endif %}
```

### `dir(path=...)` — parent directory of a path

**Both phases.** Returns the parent directory of a `/`-separated path
(`dir(path="foo/bar/baz.png")` → `"foo/bar"`). A path with no directory yields
`""`. Pair with [`filter_in_dir`](#filter_in_dir--keep-docs-in-one-directory).

## Filters

### `backlinks` — pages that link to this one

**Template phase.** Pipe an `id_path`; returns the docs whose wikilinks resolve
to it.

| Kwarg | Default | Meaning |
|-------|---------|---------|
| `order_by` | `date` | `title` \| `date` \| `updated`. |
| `sort` | `desc` | `asc` \| `desc`. |
| `omit` | `[]` | `id_path`s to exclude (e.g. `omit=[page.id_path]` drops a self-link). |
| `limit` | unlimited | Max items. |

```jinja
{% for src in page.id_path | backlinks(order_by="title", sort="asc") %}
  <li>{{ src.title }}</li>
{% endfor %}
```

### `related` — pages related to this page

**Template phase.** Pipe an `id_path`; returns the most related pages, ranked
best-match first, using the weights configured under
[`related:`](config.md#related) in `config.yaml`.

| Kwarg | Default | Meaning |
|-------|---------|---------|
| `limit` | unlimited | Max items. |
| `omit` | `[]` | `id_path`s to exclude. |

The page is always excluded from its own results; ties break by `date`
descending, then `id_path`. Weights come from config, not kwargs.

```jinja
{% for doc in page.id_path | related(limit=5) %}
  <li><a href="{{ doc.id_path | link }}">{{ doc.title }}</a></li>
{% endfor %}
```

### `entries` — iterate a map in key order

**Both phases.** Turns a map into an array of `{key, value}` objects sorted by
key (Tera's `sort` only takes arrays). `sort` is `asc` (default) or `desc`.

```jinja
{% for entry in taxonomy(name="tags") | entries(sort="desc") %}
  {{ entry.key }}: {{ entry.value | length }}
{% endfor %}
```

### `dirtree` — fold docs into a directory tree

**Both phases.** Groups an array of docs by output path and returns the content
root's children as a tree. Each node has `name` (path segment), `path`
(accumulated output path), and `kind`: directories (`"dir"`) carry `children`;
files (`"file"`) carry the original `doc`. Children sort by `name`. Walk it
with a recursive macro:

```jinja
{% macro tree(nodes) %}
<ul>
  {% for n in nodes %}
    {% if n.kind == "dir" %}
      <li>{{ n.name }}{{ self::tree(nodes=n.children) }}</li>
    {% else %}
      <li><a href="{{ n.doc.id_path | link }}">{{ n.doc.title }}</a></li>
    {% endif %}
  {% endfor %}
</ul>
{% endmacro %}

{{ self::tree(nodes=collection(name="posts") | dirtree) }}
```

### `filter_in_dir` — keep docs in one directory

**Both phases.** Keeps only the docs whose `id_path` is an **immediate** child
of `dir` (nested subdirectories excluded), sorted by `id_path`.

| Kwarg | Required | Meaning |
|-------|----------|---------|
| `dir` | yes | A literal directory; `""` for top-level docs. Not auto-derived from a file path — wrap one with `dir(...)`. |
| `omit` | no | `id_path`s to exclude. |

```jinja
{% set siblings = collection(name="all")
     | filter_in_dir(dir=dir(path=page.id_path), omit=[page.id_path]) %}
```

### `filter_by_id_path` — keep docs matching a path glob

**Both phases.** Keeps only the docs whose `id_path` matches `path`, a glob with
the same semantics as a collection [query](config.md#collections):
`literal_separator` is on, so `posts/*.md` stays within one directory while
`posts/**` descends into subdirectories. Unlike `filter_in_dir`, it **preserves
input order** — it filters, it never re-sorts.

A taxonomy term is global (it aggregates docs from every path), so this is how you
scope a shared tag to one section at render time:

| Kwarg | Required | Meaning |
|-------|----------|---------|
| `path` | yes | A glob matched against each doc's `id_path`. |
| `omit` | no | `id_path`s to exclude. |

```jinja
{% set posts = taxonomy(name="tags")["rust"] | filter_by_id_path(path="posts/**") %}
```

(For the build-phase equivalent on a taxonomy archive, see the archive
[`query:`](../guides/archives.md#scoping-a-taxonomy-archive-with-query) key.)

### `omit_docs` — drop docs from a list by `id_path`

**Both phases.** Removes docs whose `id_path` appears in `omit`, preserving
input order. The general-purpose complement to the `omit` kwargs built into
`collection`, `backlinks`, `related`, and `filter_in_dir`.

| Kwarg | Required | Meaning |
|-------|----------|---------|
| `omit` | yes | Array of `id_path` strings; an empty array is a passthrough. |

```jinja
{% set others = collection(name="all") | omit_docs(omit=[page.id_path]) %}
```

### `truncate_words` — word-aware truncation

**Both phases.** Truncates at the last whitespace that fits, appending `…` when
it cuts. Unlike Tera's built-in `truncate`, it never splits a word. Pair with
`striptags` to summarize HTML.

| Kwarg | Default | Meaning |
|-------|---------|---------|
| `length` | `250` | Max length before truncation. |

```jinja
{{ page.content | striptags | truncate_words(length=140) }}
```

### `markdown` — render Markdown to HTML

**Both phases.** Renders a string of Markdown to HTML using the same renderer
as document bodies (GitHub-flavored Markdown plus syntax-highlighted code
fences). Output is marked safe, so it is not re-escaped in `.html`/`.xml`
templates. Wikilinks and `#hashtags` are **not** processed by this filter.

```jinja
{{ page.data.blurb | markdown }}

{% filter markdown %}
Some *Markdown*, a [link](https://example.com), and a `code` span.
{% endfilter %}
```

### URL filters

**Both phases.** Four filters turn paths into URLs:

| Filter | Input | Output |
|--------|-------|--------|
| `permalink` | `id_path` | Absolute URL: `site.url` + output path. |
| `link` | `id_path` | Root-relative URL. |
| `relative_url` | any path | `base_path` + `/` + path. |
| `absolute_url` | any path | `site.url` + `base_path` + `/` + path. |

`permalink` and `link` resolve a document's `id_path` to where it actually
renders (honoring its `permalink:` frontmatter); `relative_url`/`absolute_url`
prefix arbitrary paths, e.g. static assets. When `site.url` is unset, filters
that produce absolute URLs degrade gracefully to root-relative.

```jinja
<a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
<link rel="stylesheet" href="{{ "css/style.css" | relative_url }}">
```

## Macros

Macro files in `templates/macros/` are auto-imported (non-recursively) into the
content-phase environment, so documents can call them directly. In templates,
import them explicitly with `{% import %}`. See the
[Macros guide](../guides/macros.md).

## See also

- [Templates guide](../guides/templates.md) — layouts, inheritance, a worked example
- [Configuration reference](config.md) — collections, taxonomies, related weights
- [Frontmatter reference](frontmatter.md) — where `page.*` comes from
