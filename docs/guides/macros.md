# Macros (shortcodes)

Macros are italic's shortcodes: reusable snippets — video embeds, responsive
images, callouts — that you call from Markdown. They're plain
[Tera macros](https://keats.github.io/tera/docs/#macros), so there's no
separate shortcode language to learn.

## Writing a macro

Drop a macro file in `templates/macros/`:

```html
<!-- templates/macros/youtube.html -->
{% macro embed(id) %}
<iframe src="https://www.youtube.com/embed/{{ id }}" allowfullscreen></iframe>
{% endmacro %}
```

## Calling it from content

Call it from any Markdown body, namespaced by filename:

```markdown
Here's the talk:

{{ youtube::embed(id="dQw4w9WgXcQ") }}
```

The macro expands *before* the Markdown render, so it can emit any HTML.
Macro files are auto-imported (non-recursively) into the content-phase
environment — no `{% import %}` needed in documents. In layout templates,
import them explicitly:

```jinja
{% import "macros/youtube.html" as youtube %}
```

## Content templates: the bigger picture

Macro expansion works because italic runs a full Tera render on every document
body before rendering Markdown. That means documents can also use partials,
conditionals, loops, and the page's own data:

```markdown
---
tags: ["movies", "sci-fi", "review"]
---
This post is tagged:
{% for tag in page.data.tags %} #{{ tag }}{% endfor %}
```

**The content phase has limits.** The page index doesn't exist yet while
bodies render, so inside a document you can use `page`, `site`, `data`, macros,
and the both-phase filters (`markdown`, `truncate_words`, the URL filters,
`entries`, `dirtree`, `filter_in_dir`, `omit_docs`, `dir()`) — but not the
functions and filters that read other pages (`collection()`, `all()`,
`taxonomy()`, `doc()`, `backlinks`, `related`). Those belong in layouts. See
[the build pipeline](../concepts/build-pipeline.md#consequences-worth-knowing).

## See also

- [Template reference](../reference/templates.md) — phase availability per filter
- [Templates guide](templates.md)
