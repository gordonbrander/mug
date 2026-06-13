# Templates

Layouts live in `templates/` and use
[Tera](https://keats.github.io/tera/docs/), a Jinja-style template language
with inheritance, includes, and macros. If you've used Jinja2, Liquid, or
Nunjucks, it will feel familiar.

## Assigning a template

Set a layout with the `template` frontmatter key, or via collection defaults:

```yaml
---
template: post.html
---
```

```yaml
# config.yaml — every post gets the layout without per-file frontmatter
defaults:
  posts:
    template: post.html
```

A document without a template renders its body as the final output.

## A worked base layout

```html
<!-- templates/base.html -->
<!doctype html>
<html>
<head>
  <title>{{ page.title }} | {{ site.title }}</title>
  <link rel="stylesheet" href="{{ "css/style.css" | relative_url }}">
</head>
<body>
  <main>
    <h1>{{ page.title }}</h1>
    {{ page.content | safe }}
  </main>
</body>
</html>
```

Three things to notice:

- `page.content` is the document's already-rendered HTML body — pipe it
  through `safe` so it isn't escaped.
- `site.*` is whatever you put under `site:` in `config.yaml`.
- Static assets get URL-prefixed with `relative_url` so the site works under a
  [`base_path`](permalinks.md#urls-site-url-and-base-path).

Use Tera's `{% extends %}`/`{% block %}` for layout inheritance and
`{% include %}` for partials, exactly as in the
[Tera docs](https://keats.github.io/tera/docs/#inheritance).

## Available context

Every template sees `page`, `site`, and `data`; archive pages add `pagination`
and (for taxonomy archives) `term`. The full shape is in the
[template reference](../reference/templates.md#context).

Beyond Tera's built-ins, italic adds functions and filters for listing
collections (`collection()`, `all()`), taxonomies (`taxonomy()`), graph
queries (`backlinks`, `related`), document lookup (`doc()`), list/tree
utilities (`dirtree`, `filter_in_dir`, `omit_docs`, `entries`), text helpers
(`truncate_words`, `markdown`), and URL builders (`permalink`, `link`,
`relative_url`, `absolute_url`) — all documented with examples in the
[template reference](../reference/templates.md).

## Beyond HTML

Templates can be `.html`, `.xml`, `.tera`, `.json`, or `.txt`. Only
`.html`/`.xml` are HTML-autoescaped; the others pass characters through
verbatim — which is what a JSON feed or `robots.txt` wants. `.tera` is the
generic escape hatch for any other format.

## See also

- [Template reference](../reference/templates.md) — every variable, function, and filter
- [Macros (shortcodes)](macros.md)
- [Archives, feeds & sitemaps](archives.md) — templates that generate pages
