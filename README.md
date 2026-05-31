# mug

Mug is a fast, reliable site-generator, written in Rust. Its goals are:

- Blog, wiki, and [digital garden](https://maggieappleton.com/garden-history)-aware: supports date-based posts, wikilinks, backlinks, hashtags, and more. Aims to be compatible with [Obsidian Markdown](https://obsidian.md/help/syntax), so you can easily publish your digital garden.
- Reliable: Works out-of-the-box with zero config. No framework churn, no dependencies. Does one thing well.
- Fast: it's Rust, so...

## Install

From a clone of this repo:

```sh
cargo install --path .
```

This puts `mug` on your `PATH` (typically `~/.cargo/bin/mug`).

## Quick start

```sh
mug new my-site
cd my-site
mug build       # one-shot build into public/
mug watch       # rebuild on every file change
```

`mug new` scaffolds a starter site with a sample page, a sample post, a base
template, a built-in RSS archive, and a sitemap page.

## Project layout

```
content/        Authored documents (.md, .html, .yaml). Render to their own paths.
archives/       Templates whose frontmatter declares a kind → fan out into pages.
templates/      Tera layouts, partials, and macros.
data/           YAML files mixed into the global data cascade.
static/         Copied verbatim into the build output.
config.yaml     Optional. Sensible defaults apply if absent.
```

There is no `posts/` or `pages/` convention baked in — subdirectories under
`content/` are just path prefixes. A blog is `content/posts/*.md` *by
convention of the author's queries*, not by any built-in section concept.

## Writing content

Three input types are supported:

| Type    | Frontmatter        | Body                                            |
|---------|--------------------|-------------------------------------------------|
| `.md`   | Optional YAML block | Markdown → rendered to HTML                      |
| `.html` | Optional YAML block | Raw HTML → passed through                        |
| `.yaml` | The whole file      | `content:` field rendered as HTML                |

Markdown and HTML carry frontmatter as a leading `---`-delimited YAML block:

```markdown
---
title: Hello, world
template: base.html
date: 2026-01-01
tags: [intro]
---
The body of the post goes here.
```

Recognized frontmatter keys (all optional):

| Key         | Default                                  |
|-------------|------------------------------------------|
| `title`     | `""`                                     |
| `template`  | `None` (body is the final output)        |
| `tags`      | `[]` (and any other taxonomy field — see [Taxonomies](#taxonomies)) |
| `date`      | file created time, then file modified time |
| `updated`   | file modified time                       |
| `permalink` | mirror of source path (see below)        |

Any other key is preserved verbatim on `page.data` and reachable from templates
as `{{ page.data.your_key }}`. A doc's term memberships are available as
`page.terms` (e.g. `page.terms.tags`), a map of taxonomy → slug → display text.

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

# Add default frontmatter to matching pages
# Defaults can be overridden on a per-page basis
defaults:
  "posts/*.md":
    permalink: /blog/:yyyy/:mm/:dd/:slug/
    template: post.html

# Named collections: saved queries, read in templates with collection(name=...)
collections:
  posts:
    path: "posts/*.md"
    order_by: date
    sort: desc

# Taxonomies: named classifications. Built-in `tags` is always present.
taxonomies:
  categories:
```

## Permalinks

By default a document renders to a location mirroring its source path. You can
override this by setting a `permalink` frontmatter key
(or by setting permalink defaults in your `config.yaml`).

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

Paginated archives append `page/N/` to the landing permalink for pages 2 and up
(e.g. `/blog/` → `/blog/page/2/`); there is no `:page` variable.

## Defaults

Rather than repeat the same frontmatter in every file, declare defaults for a
**collection** in `config.yaml` under a `defaults:` key. Each entry names a
collection (from `collections:`); its values fill keys the collection's members
didn't set themselves:

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
the `post.html` layout without restating either in its frontmatter. This is a
general mechanism — any frontmatter key can be defaulted, not just `permalink`.

Rules:

- **A document's own frontmatter always wins.** Defaults only fill keys the
  document left unset.
- **A `defaults:` key must name a declared collection** — an unknown name is a
  config error.
- **When several collections cover a doc, the later `defaults:` entry wins.**

Defaults are applied after collection membership is computed and before bodies
are rendered, so they behave exactly as if written inline — including feeding a
defaulted taxonomy field (e.g. `tags`) into the taxonomies.

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
- `pagination` and `term`: (only on archive-emitted pages—see below)

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

### `collection(...)` — list a named collection

Lists are defined once in `config.yaml` under `collections:` (a saved query) and
read in templates by name with `collection(name=...)`:

```yaml
# config.yaml
collections:
  posts:
    path: "posts/*.md"
    order_by: date
    sort: desc
    limit: 10
```

```jinja
{% for post in collection(name="posts") %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

A collection's query takes: `path` (glob), `order_by` (`title` | `date` |
`updated`), `sort` (`asc` | `desc`), `limit` (integer), `omit` (array of
`id_path` strings to exclude). Default is `order_by=date, sort=desc`. (Filtering
by tag is served by taxonomies — see below.)

### `taxonomy(...)` — list a taxonomy's terms

```jinja
{% for slug, docs in taxonomy(name="tags") %}
  <h2>{{ slug }}</h2>
  {% for post in docs %}<a href="{{ post.id_path | permalink }}">{{ post.title }}</a>{% endfor %}
{% endfor %}
```

Both `collection()` and `taxonomy()` classify source content only — pages
emitted by archives never appear in them.

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

Available in templates only.

### URL filters

| Filter         | Input         | Output                                |
|----------------|---------------|---------------------------------------|
| `permalink`    | id_path       | absolute URL (`site.url` + path)      |
| `link`         | id_path       | root-relative URL                     |
| `relative_url` | any path      | `base_path` + `/` + path              |
| `absolute_url` | any path      | `site.url` + `base_path` + `/` + path |

All four are available in both document bodies and templates.

## Wikilinks

In Markdown, `[[Page Title]]` and `[[Page Title|Display text]]` resolve to
pages by slugified stem. The resolver searches the current directory first,
then walks up toward the project root; the first match wins. Resolved links
render as `<a class="wikilink" href="…">…</a>`; unresolved links render as
`<span class="nolink">…</span>` and log a warning.

Every resolved wikilink also registers an edge in the backlink graph, so the
`backlinks` filter Just Works.

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

## Taxonomies

A **taxonomy** is a named classification of docs by term. The built-in `tags`
taxonomy is always present; declare more under `taxonomies:` in `config.yaml`.
Each doc lists its terms under the taxonomy's frontmatter field (the taxonomy
name by default):

```yaml
# config.yaml
taxonomies:
  categories:        # field defaults to the taxonomy name
  series:
    field: serie     # override the frontmatter field
```

The built-in `tags` is merged in, not replaced — opt out with `tags: false`. A
doc's memberships live on `page.terms` (e.g. `page.terms.categories`), and
`taxonomy(name=...)` returns a term-slug → docs map for templates and archives.

## Archives

An **archive** is a template in `archives/` that fans a collection or taxonomy
out into many output pages. Its frontmatter declares a `kind` and names the
collection/taxonomy; the body renders once per page with a `pagination` context.
The `permalink` is the page-1 (landing) URL — pages 2+ append `page/N/`
automatically (e.g. `/blog/` → `/blog/page/2/`).

Paginate a collection into list pages — `archives/blog.html`:

```yaml
---
kind: collection
collection: posts
permalink: /blog/
per_page: 10
template: blog-layout.html
---
{% for post in pagination.items %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

Emit one (optionally paginated) page per taxonomy term — `:term` in the
`permalink` is the term slug, and the body receives a `term` (`slug`, `text`):

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

A whole-site listing such as `sitemap.xml` is an ordinary `content/` page that
iterates a collection, not an archive — it lists canonical pages, not paginated
archive pages. The scaffold ships a starter RSS archive and a sitemap page that
work out of the box.

## CLI

| Command                | Purpose                                          |
|------------------------|--------------------------------------------------|
| `mug build`          | Run the full pipeline once into `output_dir`.    |
| `mug watch`          | Rebuild on every change to a source dir or `config.yaml` (~150 ms debounce). |
| `mug new <path>`     | Scaffold a starter site at `<path>` (must not exist). |
| `mug clean`          | Remove `output_dir` (default `public`).          |

All behavioral configuration lives in files, not flags.

## Scope and limits (v1)

- **Full-rebuild only.** Every `watch` event triggers a full rebuild. The query
  model is fundamentally at odds with cheap incremental builds.
- **No asset pipeline.** `static/` is copied verbatim. No bundling, no
  minification, no fingerprinting.
- **Markdown and raw HTML only.** No reStructuredText, AsciiDoc, etc.
- **Tera macros are the only extension point.** No embedded scripting.
