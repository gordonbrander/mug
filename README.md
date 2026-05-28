# mug

A zero-config static site generator written in Rust. Every site-specific
behavior lives in `config.yaml`, frontmatter, and [Tera] templates — never in
code or scripts.

[Tera]: https://keats.github.io/tera/

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
template, and built-in RSS and sitemap generators.

## Project layout

```
content/        Authored documents (.md, .html, .yaml). Render to their own paths.
generators/     Templates whose frontmatter declares a query → fan out into pages.
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
| `tags`      | `[]`                                     |
| `date`      | file created time, then file modified time |
| `updated`   | file modified time                       |
| `permalink` | mirror of source path (see below)        |

Any other key is preserved verbatim on `doc.data` and reachable from templates
as `{{ doc.data.your_key }}`.

## Permalinks

By default a document renders to a location mirroring its source path. The
`permalink` frontmatter key overrides this with a path template expanded
against:

- `:slug` — sluggified stem of the document
- `:yyyy`, `:mm`, `:dd` — components of `date`
- `:page` — page number (generators with pagination only)

A trailing `/` writes `index.html`:

```yaml
permalink: /blog/:yyyy/:slug/   # → /blog/2026/hello/index.html
```

## Templates

Templates live in `templates/` and use [Tera]. A document picks one with
`template: name.html` in its frontmatter.

Inside a template, the available context is:

- `doc` — the current document (`doc.title`, `doc.tags`, `doc.date`, …, plus
  `doc.data` for arbitrary frontmatter)
- `page.content` — the document's rendered body
- `site` — the `site:` submap from `config.yaml`
- `data` — every top-level YAML file in `data/`, keyed by filename stem
- `pagination` — only on generator-emitted pages (see below)

Example `templates/base.html`:

```html
<!doctype html>
<html>
<head><title>{{ doc.title }} | {{ site.title }}</title></head>
<body>
  <main>{{ page.content | safe }}</main>
</body>
</html>
```

## Filters and functions

### `query(...)` — list documents

Iterate over the in-memory index. Available in **templates only**, not in
document bodies.

```jinja
{% for post in query(path="posts/*.md", order_by="date", sort="desc", limit=10) %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

Kwargs: `path` (glob), `tag` (string), `order_by` (`title` | `date` |
`updated`), `sort` (`asc` | `desc`), `limit` (integer). Default is
`order_by=date, sort=desc`.

### `backlinks` — pages that link to this one

```jinja
{% for src in doc.id_path | backlinks(order_by="title", sort="asc") %}
  <li>{{ src.title }}</li>
{% endfor %}
```

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

## Generators

A generator is a template in `generators/` whose frontmatter describes a
query. The build expands the generator into zero or more virtual pages that
join the index alongside authored content.

`generators/blog.html`:

```yaml
---
permalink: /blog/page-:page/
per_page: 10
template: blog-layout.html
query:
  path: posts/*.md
  order_by: date
  sort: desc
---
{% for post in pagination.items %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

Pagination context (`pagination.current`, `pagination.total`,
`pagination.prev_url`, `pagination.next_url`, `pagination.items`) is injected
automatically.

The `weight` key controls generator execution order — generators that must
see the output of *other* generators (e.g. a sitemap) set a high value:

```yaml
permalink: /sitemap.xml
weight: 9999
```

The scaffold ships starter RSS and sitemap generators that work out of the
box.

## `config.yaml`

All keys are optional. Defaults shown:

```yaml
content_dir: content
output_dir: public
templates_dir: templates
static_dir: static
data_dir: data
generators_dir: generators

site:
  # Anything under `site:` is reachable in templates as `{{ site.x }}`.
  title: My Site
  description: A site built with mug.
  url: https://example.com   # origin for absolute URLs; no trailing slash
  base_path: ""              # subpath the site is hosted under, e.g. "/blog"
```

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
