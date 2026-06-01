# mug

Mug is a site-generator written in Rust. Its goals are:

- Practical: Works out-of-the-box with zero config. One binary with everything you need.
- Flexible: supports blogs, websites, and [digital gardens](https://maggieappleton.com/garden-history).
- Fast: Embarrasingly parallel rendering with Rust.

## Features

Mug has everything you need for publishing blogs, websites, and [Digital gardens](https://maggieappleton.com/garden-history)...

- Blogs: Create any number of blogs or newsfeeds on the same site.
- Custom collections: A poweful query system lets you collect pages into any grouping you want.
- Multiple taxonomies: Organize your content along multiple axes. Want to categorize by tag? Series? Publication? Phase of the moon? No problem.
- Archives: Generate custom paginated archives for taxonomies and collections.
- Fancy Markdown: Aims to be maximally compatible with GitHub-flavored Markdown and [Obsidian Markdown](https://obsidian.md/help/syntax), so you can easily publish your vault.
- Wikilinks: smart wikilinks that resolve using the same algorithm as Obsidian.
- Backlinks: list pages that link to a page.
- Hashtags: auto-appended to tags and stripped from output.
- Shortcodes: easily create custom shortcodes for video embeds, responsive images, and more.
- Content templates: Use Tera templates in Markdown.
- Drafts: mark a page `draft: true` to keep it out of your published site while still previewing it locally.
- RSS feeds
- Sitemaps

## Install

From a clone of this repo:

```sh
cargo install --path .
```

This puts `mug` on your `PATH` (typically `~/.cargo/bin/mug`).

## Quick start

`mug new` scaffolds a starter site with a sample page, a sample post, a base
template, a built-in RSS archive, and a sitemap page.

```sh
mug new my-site
cd my-site
mug serve       # Start a dev server, automatically rebuild on change
```

## Project layout

```
content/        # Your site content (.md, .html, .yaml)
archives/       # Generated archives (tags, collections, feeds, sitemaps, etc)
templates/      # Tera layouts, partials, and macros.
data/           # YAML files mixed into the global data cascade.
static/         # Copied verbatim
config.yaml     # Site config
```

Note that Mug doesn't impose a specific layout on your content folder. You can organize
it however you you like, and use custom **collections** to define blogs, sections, and
other concepts. This flexibility lets you support multiple blogs, news feeds, and
portals in the same site.

## Authoring content

Mug supports three kinds of content:

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
if the file weren't there. They are visible while you work locally: `mug serve`
and `mug watch` always include drafts. To preview drafts in a one-off build
(e.g. a staging deploy), pass `mug build --drafts`.

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

site:
  # Anything under `site:` is reachable in templates as `{{ site.x }}`.
  title: My Site
  description: A site built with mug.
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

# Add default frontmatter to collections
# Defaults can be overridden on a per-page basis
defaults:
  posts:
    permalink: /blog/:yyyy/:mm/:dd/:slug/
    template: post.html

# Extract inline `#hashtags` from Markdown bodies into the `tags` taxonomy.
hashtags: true
```

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
- `limit`: Max number of items in this collection. Defaults to "unlimited".
- `omit`: a list of documents to exclude (by `id_path`).

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
under `taxonomies:` in `config.yaml`. These fields will be treated as tags by Mug.

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

When hashtags are turned on (`hashtags: true` in `config.yaml`), Mug will lift inline `#hashtags` into
the `tags` taxonomy and strip them from the rendered markup.

## Templates

Templates live in `templates/` and use [Tera](https://keats.github.io/tera/docs/), a
Jinja-style templating system. Set a template with the `template` frontmatter key
(or via defaults in `config.yaml`):

```yaml
template: post.html
```

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
plus a few extra added by mug...

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
    limit: 10
```

```jinja
{% for post in collection(name="recent_posts") %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

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
drop a page's self-link from its own backlinks). Default is
`order_by=date, sort=desc`.

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

Mug runs an initial Tera template render on content **before** rendering markup
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

Pagination context (`pagination.current`, `pagination.total`,
`pagination.prev_url`, `pagination.next_url`, `pagination.items`) is injected
automatically. Archives read only the classification of source content (never
each other's output), so they are order-independent and run in parallel — there
is no execution-order key.

The scaffold ships a starter RSS archive and a sitemap page that work out of the box.

## CLI

| Command                | Purpose                                          |
|------------------------|--------------------------------------------------|
| `mug build`          | Run the full pipeline once into `output_dir`. Excludes drafts; pass `--drafts` to include them. |
| `mug watch`          | Rebuild on every change to a source dir or `config.yaml` (~150 ms debounce). Includes drafts. |
| `mug new <path>`     | Scaffold a starter site at `<path>` (must not exist). |
| `mug clean`          | Remove `output_dir` (default `public`).          |

Behavioral configuration lives in files, not flags — the one exception is
`mug build --drafts`, which force-includes [drafts](#drafts) in a build.

## Scope and limits (v1)

- **Full-rebuild only.** Every `watch` event triggers a full rebuild. The query
  model is fundamentally at odds with cheap incremental builds.
- **No asset pipeline.** `static/` is copied verbatim. No bundling, no
  minification, no fingerprinting.
- **Markdown and raw HTML only.** No reStructuredText, AsciiDoc, etc.
- **Tera macros are the only extension point.** No embedded scripting.
