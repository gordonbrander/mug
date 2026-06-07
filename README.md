# Italic

A static site generator for [digital gardens](https://maggieappleton.com/garden-history).

- Built for thinkers: wikilinks, backlinks, custom collections, related notes, custom taxonomies, and more.
- Batteries included: One binary with everything you need. Zero config required.
- Fast: Build thousands of pages in < 1s. Written in Rust with an embarrassingly parallel rendering pipeline.

## Features

Italic has a number of features that make it easy publish a digital garden from your [Obsidian Vault](https://obsidian.md/), or any other folder full of Markdown.

- Markdown extensions: compatible with [GitHub-flavored Markdown](https://github.github.com/gfm/) and [Obsidian Markdown](https://obsidian.md/help/syntax)
- Wikilinks: does fuzzy link matching using the same algorithm as Obsidian.
- Backlinks: see what links into a page
- Hashtags: auto-appended to tags and stripped from output.
- Related: surface related pages. Uses a [TF-IDF](https://en.wikipedia.org/wiki/Tf%E2%80%93idf) algorithm over taxonomy and backlinks.

It also has all of the other things you might expect from a static site generator, plus a few extras:

- Blog-aware: publish multiple blogs from the same site.
- Custom collections: A poweful query system lets you collect pages into any grouping you want.
- Multiple taxonomies: Want to categorize by tag? Series? Publication? Phase of the moon? No problem.
- Themes: customize the look and feel of your site.
- Powerful templates: uses [Tera](https://keats.github.io/tera/docs), a familiar template syntax with template extensions, macros and more.
- Shortcodes: easily create [Tera macros](https://keats.github.io/tera/docs/#macros) for video embeds, responsive images, and more.
- Archives
- Drafts
- RSS feeds
- Sitemaps
- ...and more

## Install

From a clone of this repo:

```sh
cargo install italic
```

This puts `italic` on your `PATH` (typically `~/.cargo/bin/italic`).

## Quick start

```sh
italic new my-site
cd my-site
echo '# Heading' > content/index.md
italic serve
```

Congrats! You have a website: <https://localhost:3000>.

However, this website is pretty basic. We can add some custom templates, or we can get off the ground by downloading some starter themes:

```sh
git clone --depth 1 https://github.com/gordonbrander/italic_themes.git themes/
```

Set a theme by adding the `theme` key to your `config.yaml`: 

```yaml
# config.yaml
theme: "themes/obsidian"
```

If you want, you can run `italic scaffold` to add some demo content for the theme.

That's it! Have fun building your new website.

## CLI

| Commanud                | Purpose                                          |
|------------------------|--------------------------------------------------|
| `italic build`          | Run the full pipeline once into `output_dir`. Excludes drafts; pass `--drafts` to include them. |
| `italic serve`          | Serve + rebuild on every change. Includes drafts. |
| `italic watch`          | Rebuild on every change (no server). |
| `italic new <path>`     | Scaffold an empty starter site at `<path>`. |
| `italic scaffold`       | Copy the configured theme's starter content into `content/` (skips existing files). |
| `italic clean`          | Remove `output_dir` (default `public`).          |

## Project layout

```
content/        # Your site content (.md, .html, .yaml)
archives/       # Generated archives (tags, collections, feeds, sitemaps, etc)
templates/      # Tera layouts, partials, and macros.
data/           # YAML files mixed into the global data cascade.
static/         # Copied verbatim
themes/         # Themes you reference via `theme:` in config.yaml
config.yaml     # Site config
```

Note that Italic doesn't impose a specific layout on your content folder. You can organize
it however you you like, and use custom **collections** to define blogs, sections, and
other concepts. This flexibility lets you support multiple blogs, news feeds, and
portals in the same site.

## Authoring content

Italic supports three kinds of content:

| Type    | Frontmatter        | Body                                            |
|---------|--------------------|-------------------------------------------------|
| `.md`   | Optional YAML block | Markdown → rendered to HTML                      |
| `.html` | Optional YAML block | Raw HTML → passed through                        |
| `.yaml` | The whole file      | `content:` field rendered as HTML                |

Markdown and both HTML allow you to add frontmatter for structured data:

```markdown
---
title: Hello, world
template: base.html
date: 2026-01-01
tags: [intro]
---
The body of the post goes here.
```

A few frontmatter keys have special meaning, and are given sensible defaults
if absent:

| Key         | Default                                  |
|-------------|------------------------------------------|
| `title`     | `""`                                     |
| `draft`     | `false` (see [drafts](#drafts))          |
| `template`  | `None` (body is the final output)        |
| `tags`      | `[]` (and other taxonomy fields—see [taxonomies](#taxonomies)) |
| `date`      | file created time, then file modified time |
| `updated`   | file modified time                       |
| `permalink` | mirror of source path (see below)        |

Any other key is preserved verbatim on `page.data` and reachable from templates
as `{{ page.data.your_key }}`. A doc's term memberships are available as
`page.terms` (e.g. `page.terms.tags`), a map of taxonomy → slug → display text.

### Drafts

Mark a page as a draft by setting `draft: true` in its frontmatter:

```markdown
---
title: Work in progress
draft: true
---
Not ready to publish yet.
```

Drafts are dropped at the start of the build, so they never appear in the
output — and never show up in collections, taxonomies, or backlinks either, as
if the file weren't there. They are visible while you work locally: `italic serve`
and `italic watch` always include drafts. To preview drafts in a one-off build
(e.g. a staging deploy), pass `italic build --drafts`.

### Wikilinks

In Markdown, `[[Page Title]]` and `[[Page Title|Display text]]` resolve to
pages by slugified stem. The resolver uses the same algorithm as Obsidian,
searching the current directory first, expanding the search until it finds the
closest match.

Resolved links render as `<a class="wikilink" href="…">…</a>`; unresolved
links render as `<span class="nolink">…</span>`.

Every resolved wikilink also registers an edge in the page's backlink graph.

## Site config (`config.yaml`)

Site-wide configuration goes in `config.yaml`. All keys are optional and come with sensible defaults.

```yaml
content_dir: content
output_dir: public
templates_dir: templates
static_dir: static
data_dir: data
archives_dir: archives

# Optional: layer a theme (see "Themes" below). No default.
# theme: themes/my-theme

site:
  # Anything under `site:` is reachable in templates as `{{ site.x }}`.
  title: My Site
  description: A site built with italic.
  url: https://example.com   # origin for absolute URLs; no trailing slash
  base_path: ""              # subpath the site is hosted under, e.g. "/blog"

# Collections are saved queries.
# You can access them in templates with collection(name=...)
collections:
  posts:
    path: "posts/*.md"
    order_by: date
    sort: desc

# Taxonomies are custom tag and category types.
# Defined by listing the frontmatter fields you want to be treated as taxonomies.
taxonomies:
  - tags
  - category

# Tune the related() filter: how much each namespace counts toward relatedness.
# Keys are taxonomies. `links` is a special key that represents relatedness by
# wikilink graph (links, backlinks, and co-citations).
# Default: equal weight on every key.
related:
  weights:
    tags: 2.0
    links: 1.0

# Add default frontmatter to collections
# Defaults can be overridden on a per-page basis
defaults:
  posts:
    permalink: /blog/:yyyy/:mm/:dd/:slug/
    template: post.html

# Extract inline `#hashtags` from Markdown bodies into the `tags` taxonomy.
hashtags: true
```

## Themes

A **theme** bundles templates, archives, static assets, and config defaults in a
folder, so a whole look-and-feel can be shared and reused. Point at one with the
top-level `theme:` key:

```yaml
# config.yaml
theme: themes/my-theme
```

A theme is just a folder laid out like a site — its own optional `config.yaml`
plus the conventional subdirs:

```
themes/my-theme/
  config.yaml     # theme's config defaults (optional)
  templates/      # Tera layouts, partials, macros
  archives/       # collection/taxonomy archive pages
  static/         # static assets
```

When a theme is set, Italic layers it underneath your site:

- **Templates and archives** come from the theme. Your site's own `templates/`
  and `archives/` directories are not used — customize the look through config
  and the static overlay instead. A theme always uses the conventional
  `templates/`, `archives/`, and `static/` subdir names relative to its root;
  the `*_dir` keys in a theme's own `config.yaml` do not apply to it.
- **Config** in the theme's `config.yaml` provides **defaults** your site
  overrides. `collections` and `defaults` merge by name (your site wins on a
  name clash, the theme's other entries are kept); `taxonomies` are unioned; the
  `site:` map is deep-merged with your values winning.
- **Static** is overlaid: the theme's `static/` is copied first, then your
  site's `static/` over the top, so your files win on a path collision.
- **`data/`, `content/`, and the output directory stay yours** — a theme never
  ships data or content, nor dictates where your content lives or output goes.

A theme without a `config.yaml` still contributes its files. Themes don't nest:
a `theme:` key inside a theme's own `config.yaml` is ignored.

Themes live outside your project — a theme is just a directory with the layout
above. Reference one by path with `theme:` in `config.yaml`, then run
`italic scaffold` to copy its starter content into `content/`. `italic new` ships
no theme; bring your own or point at a shared one.

## Permalinks

By default a document renders to a location mirroring its source path. You can
override this by setting a `permalink` frontmatter key
(or by setting a permalink default in your `config.yaml`).

```yaml
permalink: /blog/:yyyy/:slug/   # → /blog/2026/hello/index.html
```

(A trailing `/` writes `index.html`)

Available permalink variables:

- `:slug` — sluggified stem of the document
- `:yyyy`: year
- `:mm`: two-digit month
- `:dd`: two-digit day
- `:term` — term slug (taxonomy archives only)

## Collections

Collections are defined in `config.yaml` and let you create custom groups and sections
for your site. For example, you can define a blog like this:

```yaml
collections:
  posts:
    path: "posts/*.md"
    order_by: date
    sort: desc
```

This gives you a reverse-chronological collection of posts that can be accessed
in templates and used to generate archives. You can define as many collections as you want.

Collection queries can specify:

- `path`: A glob pattern for matching files in `content/`.
- `order_by`: The field to sort by. Can be `title`, `date`, or `updated`. Default: `date`.
- `sort`: The direction of the sort. Can be `asc` or `desc`. Default: `desc`.
- `omit`: a list of specific documents to exclude (by `id_path`).

## Defaults

Rather than repeating the same frontmatter for every file, you can **set defaults for a
collection** in `config.yaml`.

```yaml
collections:
  posts:
    path: "posts/*.md"

defaults:
  posts:
    permalink: /blog/:yyyy/:mm/:dd/:slug/
    template: post.html
```

With the above, every member of the `posts` collection gets a dated permalink and
the `post.html` layout without having to write either in its frontmatter. When a document
belongs to more than one collection, and matches more than one default, the last default
wins. Of course, the document's own frontmatter always overrides defaults.


## Taxonomies

Taxonomies let you categorize docs. Declare taxonomies as an array of fields
under `taxonomies:` in `config.yaml`. These fields will be treated as tags by Italic.

```yaml
# config.yaml
taxonomies:
  - tags
  - category
  - series
```

```yaml
# a document's frontmatter
category: [rust, tools]
```

You can define as many taxonomies as you like. This can be a powerful way to organize
content on complex websites.

When hashtags are turned on (`hashtags: true` in `config.yaml`), Italic will lift inline `#hashtags` into
the `tags` taxonomy and strip them from the rendered markup.

## Related pages

Italic can surface the pages most **related** to a given page — the heart of a
digital garden. Relatedness is **weighted shared-term overlap**: two pages are
related in proportion to how much they have in common, across two kinds of
namespace:

- **Taxonomies** — pages that share terms (two notes tagged `phenomenology`).
- **`links`** — the **whole wikilink graph**, in both directions. This is
  broader than the [`backlinks`](#backlinks--pages-that-link-to-this-one) filter
  (which is incoming links only): a single symmetric measure relates two pages
  when **any** of these hold —
  - one page **links to** the other (an outbound link), *or*
  - one page is **linked to by** the other (a backlink), *or*
  - both pages **link to the same third page** (a shared reference).

  Because it's symmetric, if it relates A to B it also relates B to A.

Each namespace carries a `weight` you set under `related:` in `config.yaml`, so
you can decide whether a shared tag counts for more or less than a shared link:

```yaml
related:
  weights:
    tags: 2.0      # a taxonomy: shared tags
    series: 1.0    # any declared taxonomy can be weighted
    links: 1.0     # the whole link graph (both directions; see above)
```

`weights` is the only key — the whole `related:` block is optional. With no
block, every declared taxonomy and the `links` graph get equal weight, so it
works zero-config: relating by `links`, and by `tags` (and any other taxonomy)
once you declare it. A page is never related to itself, and results are ranked
best-match first.

Read the related pages in a template with the [`related`](#related--pages-related-to-this-page)
filter.

## Templates

Templates live in `templates/` and use [Tera](https://keats.github.io/tera/docs/), a
Jinja-style templating system. Set a template with the `template` frontmatter key
(or via defaults in `config.yaml`):

```yaml
template: post.html
```

Templates are any `.html`, `.xml`, `.tera`, `.json`, or `.txt` file under
`templates/`. Use `.json`/`.txt` to template those formats directly (a JSON feed,
a `robots.txt`), or `.tera` as a generic escape hatch for any other format. Only
`.html`/`.xml` are HTML-autoescaped; in `.tera`/`.json`/`.txt` templates characters
like `&`, `<`, and `/` pass through verbatim (which is what JSON and plain text want).

Inside a template, the available context is:

- `page`: the current document (`page.title`, `page.terms`, `page.date`, …,
  `page.content` for the rendered body, plus `page.data` for full frontmatter)
- `site`: the `site:` submap from `config.yaml`
- `data`: every top-level YAML file in `data/`, keyed by filename stem
- `pagination` and `term`: (only on archive pages—see below)

Example `templates/base.html`:

```html
<!doctype html>
<html>
<head><title>{{ page.title }} | {{ site.title }}</title></head>
<body>
  <main>{{ page.content | safe }}</main>
</body>
</html>
```

## Template filters and functions

Templates get all the [built-in Tera template filters and functions](https://keats.github.io/tera/docs/#built-ins),
plus a few extra added by italic...

### `collection(...)` — list a named collection

Collections are defined in `config.yaml` under `collections:` and
accessible in templates via `collection(name=...)`.

For example:

```yaml
# config.yaml
collections:
  recent_posts:
    path: "posts/*.md"
    order_by: date
    sort: desc
```

```jinja
{% for post in collection(name="recent_posts", limit=10) %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

Kwargs: `name` (required), plus optional `omit` (array of `id_path` strings to
exclude) and `limit` (max items). `omit` layers *on top of* the collection's own
definition-time `omit`; `limit` is a render-time cap (a collection has no
definition-time count — that's deliberately the filter's job). The cached result
is filtered then truncated, with `omit` applied before `limit`. Handy when a page
wants to exclude itself from a collection it belongs to:

```jinja
{% for post in collection(name="recent_posts", omit=[page.id_path], limit=5) %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

Available in: template phase.

### `all()` — list every doc

Returns every document on the site, with no `config.yaml` setup. Useful for a
sitemap, a search index, or a flat archive:

```jinja
{% for doc in all() %}
  <a href="{{ doc.id_path | link }}">{{ doc.title }}</a>
{% endfor %}
```

Docs come back in `id_path` order. `all()` takes **no arguments** — to order,
limit, or filter, define a [collection](#collections) (or pipe the result
through array filters like [`omit_docs`](#omit_docs--drop-docs-from-a-list-by-id_path),
[`dirtree`](#dirtree--fold-docs-into-a-directory-tree), or Tera's built-in
`slice`). Passing any argument is an error rather than a silent no-op.

Available in: template phase.

### `taxonomy(...)` — list a taxonomy's terms

```jinja
{% for slug, docs in taxonomy(name="tags") %}
  <h2>{{ slug }}</h2>
  {% for post in docs %}<a href="{{ post.id_path | permalink }}">{{ post.title }}</a>{% endfor %}
{% endfor %}
```

Available in: template phase.

### `backlinks` — pages that link to this one

```jinja
{% for src in page.id_path | backlinks(order_by="title", sort="asc") %}
  <li>{{ src.title }}</li>
{% endfor %}
```

Kwargs: `order_by` (`title` | `date` | `updated`), `sort` (`asc` | `desc`),
`omit` (array of `id_path` strings to exclude — e.g. `omit=[page.id_path]` to
drop a page's self-link from its own backlinks), and `limit` (max items).
Default is `order_by=date, sort=desc`.

Available in: template phase.

### `related` — pages related to this page

Lists the pages most related to a page, ranked best-match first, using the
weights configured under [`related:`](#related-pages) in `config.yaml`:

```jinja
{% for doc in page.id_path | related(limit=5) %}
  <li><a href="{{ doc.id_path | link }}">{{ doc.title }}</a></li>
{% endfor %}
```

Kwargs: `limit` (max items, default unlimited) and `omit` (array of `id_path`
strings to exclude) — both set per call, not in config. The page is always
excluded from its own results; ties break by `date` desc then `id_path`. The
per-namespace `weights` come from config, not kwargs.

Available in: template phase.

### `doc(...)` — look up a single doc

Fetch one document by its `id_path`. Returns `null` for an unknown path (so you
can guard with `{% if %}` rather than failing the build):

```jinja
{% set about = doc(id_path="about.md") %}
{% if about %}<a href="{{ about.id_path | link }}">{{ about.title }}</a>{% endif %}
```

Available in: template phase.

### `entries` — iterate a map in key order

Tera's `sort` filter only takes arrays. `map | entries` turns a map into an
array of `{key, value}` objects sorted by key — handy for walking a
`taxonomy(...)` map deterministically. `sort` is `asc` (default) or `desc`:

```jinja
{% for entry in taxonomy(name="tags") | entries(sort="desc") %}
  {{ entry.key }}: {{ entry.value | length }}
{% endfor %}
```

Available in: template phase, content phase.

### `dirtree` — fold docs into a directory tree

`docs | dirtree` groups an array of docs by their output path and returns the
content root's children as a tree, so you can render docs as a hierarchy
(sitemap, archive index, file-browser nav) instead of a flat list. Each node is
either a directory (`kind: "dir"`, with `children`) or a file (`kind: "file"`,
with the original `doc`); both carry a `name` (the path segment) and a `path`
(the accumulated output path). Children are sorted by `name`. Walk it with a
recursive macro:

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

Available in: template phase, content phase.

### `dir(...)` — parent directory of a path

`dir(path="foo/bar/baz.png")` returns the parent directory of a `/`-separated
path (`foo/bar`). A path with no directory (`baz.png`) yields an empty string.
Pair it with `filter_in_dir` to derive a directory from a page's `id_path`:

```jinja
{{ dir(path=page.id_path) }}
```

Available in: template phase, content phase.

### `filter_in_dir` — keep docs in one directory

`docs | filter_in_dir(dir="...")` keeps only the docs whose `id_path` is an
*immediate* child of `dir` (nested subdirectories are excluded), sorted by
`id_path`. Combine it with `dir(...)` to list a page's siblings — the docs that
share its directory:

```jinja
{% set siblings = collection(name="all")
     | filter_in_dir(dir=dir(path=page.id_path), omit=[page.id_path]) %}
{% for doc in siblings %}
  <a href="{{ doc.id_path | link }}">{{ doc.title }}</a>
{% endfor %}
```

Kwargs: `dir` (required — a literal directory; use `""` for top-level docs) and
`omit` (array of `id_path` strings to exclude, e.g. `omit=[page.id_path]` to drop
the page itself). `dir` is not auto-derived from a file path; wrap one with
`dir(...)`.

Available in: template phase, content phase.

### `omit_docs` — drop docs from a list by `id_path`

`docs | omit_docs(omit=[...])` removes the docs whose `id_path` appears in
`omit`, preserving the input order. It's the general-purpose complement to the
`omit` kwarg built into `collection`, `backlinks`, `related`, and
`filter_in_dir` — reach for it on any list those don't cover (a `dirtree` input,
a concatenation, or dropping the current page from a hand-built array):

```jinja
{% set others = collection(name="all") | omit_docs(omit=[page.id_path]) %}
```

Kwargs: `omit` (required — an array of `id_path` strings; an empty array is a
passthrough).

Available in: template phase, content phase.

### `truncate_words` — word-aware truncation

`text | truncate_words(length=N)` truncates at the last whitespace that fits,
appending `…` when it cuts. Default `length` is 250. Unlike Tera's built-in
`truncate`, it never splits a word; pair with `striptags` to summarize HTML.

Available in: template phase, content phase.

### `markdown` — render Markdown to HTML

Render a string of Markdown to HTML. Use the block form to render a whole
region, or the pipe form to render a value:

```jinja
{% filter markdown %}
# Hello

Some *Markdown*, a [link](https://example.com), and a `code` span.
{% endfilter %}
```

```jinja
{{ page.data.blurb | markdown }}
```

Uses the same renderer as Markdown bodies (GitHub-flavored Markdown plus
syntax-highlighted code fences), and its output is marked safe, so it is not
re-escaped in `.html`/`.xml` templates. Wikilinks and `#hashtags` are not
rendered in this filter (since the page index is unavailable during the content phase).

Available in: template phase, content phase.

### URL filters

| Filter         | Input         | Output                                |
|----------------|---------------|---------------------------------------|
| `permalink`    | id_path       | absolute URL (`site.url` + path)      |
| `link`         | id_path       | root-relative URL                     |
| `relative_url` | any path      | `base_path` + `/` + path              |
| `absolute_url` | any path      | `site.url` + `base_path` + `/` + path |

Available in: template phase, content phase.

## Macros (shortcodes)

Drop a Tera macro file in `templates/macros/`:

```html
<!-- templates/macros/youtube.html -->
{% macro embed(id) %}
<iframe src="https://www.youtube.com/embed/{{ id }}" allowfullscreen></iframe>
{% endmacro %}
```

Call it from any Markdown body — it expands *before* Markdown render:

```markdown
{{ youtube::embed(id="dQw4w9WgXcQ") }}
```

Macro files are auto-imported (non-recursively) into the markup-phase Tera
environment. In templates, import them explicitly with `{% import %}`.

## Content templates

Italic runs an initial Tera template render on content **before** rendering markup
and templates. This is what enables macros, and it also means you can use
Tera partials and other features in your docs:

```markdown
---
tags: ["movies", "sci-fi", "review"]
---

This post has tags:

{% for tag of page.tags %}
  {{ tag }}
{% endfor %}
```

Within the content phase, Tera templates can't access data from other pages,
only site data and data from the page they render in.

## Archives

An **archive** is a template in `archives/` that genenerates output pages from
a collection or taxonomy. Archives are used to generate paginated collection
archives and tag archives, as well as things like RSS feeds and sitemaps.

Archives come in several `kind`s (e.g. "taxonomy" or "collection").
The body of the archive template renders once per page with a `pagination` context.
When paginated, `permalink` has page numbers appended automatically
(e.g. `/blog/` → `/blog/page/2/`).

Example: `archives/blog.html`:

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

Example: `tag-archive.html`: Emit one (optionally paginated) page per taxonomy term
— `:term` in the `permalink` is the term slug, and the body receives a `term` (`slug`, `text`):

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

An archive can also cap how many items it covers with an optional `limit:`,
useful when an archive references a collection/taxonomy by name and can't pass a
render-time argument:

```yaml
---
kind: collection
collection: posts
permalink: /blog/
limit: 100      # paginate at most the first 100 items…
per_page: 10    # …10 per page → 10 pages
---
```

`limit` and `per_page` are independent and compose: `limit` caps the item set,
then `per_page` splits that capped set into pages (so `limit: 100, per_page: 10`
yields 10 pages, not one big page). For a **collection** archive `limit` caps the
total; for a **taxonomy** archive (one page-set per term) it caps items *per
term*. "First N" follows the collection's query order, or date-desc for a
taxonomy.

A `pagination` context is injected into every archive page automatically:

| Field                  | Meaning                                                    |
|------------------------|------------------------------------------------------------|
| `pagination.items`     | The docs on this page                                      |
| `pagination.current`   | Current page number (1-indexed)                            |
| `pagination.total`     | Total number of pages                                      |
| `pagination.prev_url`  | URL of the previous page, or unset on the first page       |
| `pagination.next_url`  | URL of the next page, or unset on the last page            |

Because `prev_url`/`next_url` are unset (rather than empty) at the ends, you can
test for them directly to render prev/next navigation that only appears when
there's somewhere to go:

```html
<nav class="pagination">
  {% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">← Previous</a>{% endif %}
  <span>Page {{ pagination.current }} of {{ pagination.total }}</span>
  {% if pagination.next_url %}<a href="{{ pagination.next_url }}">Next →</a>{% endif %}
</nav>
```

Archives read only the classification of source content (never each other's
output), so they are order-independent and run in parallel — there is no
execution-order key.
