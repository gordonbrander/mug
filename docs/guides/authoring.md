# Authoring content

Write pages as plain files in `content/` — Markdown for prose, HTML when you
need exact markup, YAML for data-heavy pages. No special structure is required;
organize the folder however you like.

## Markdown flavor

Italic renders [GitHub-flavored Markdown](https://github.github.com/gfm/) and
is compatible with [Obsidian Markdown](https://help.obsidian.md/syntax),
including:

- `[[Wikilinks]]` with Obsidian-style fuzzy resolution — see
  [Wikilinks & backlinks](wikilinks.md)
- Inline `#hashtags` lifted into your tags (when enabled) — see
  [Taxonomies & hashtags](taxonomies.md)
- Syntax-highlighted code fences

## The three content types

| Type | Frontmatter | Body |
|------|-------------|------|
| `.md` | Optional YAML block | Markdown → rendered to HTML |
| `.html` | Optional YAML block | Raw HTML → passed through |
| `.yaml` | The whole file | `content:` field rendered as HTML |

## Frontmatter

Add structured data to any document with a YAML block:

```markdown
---
title: Hello, world
template: base.html
date: 2026-01-01
tags: [intro]
---
The body of the post goes here.
```

Everything is optional. `title`, `date`, `template`, and friends have sensible
defaults (dates fall back to file timestamps); any key italic doesn't recognize
is kept verbatim and reachable in templates as `{{ page.data.your_key }}`. The
full key list and default rules are in the
[frontmatter reference](../reference/frontmatter.md).

Repeating yourself? Set frontmatter
[defaults per collection](collections.md#defaults) in `config.yaml` instead of
on every file.

## Templates inside content

Document bodies are themselves rendered by Tera before the Markdown render, so
you can use macros, partials, and the page's own data inline:

```markdown
---
tags: ["movies", "sci-fi", "review"]
---
This post is tagged:
{% for tag in page.data.tags %} {{ tag }}{% endfor %}
```

Within this content phase a template sees only site data and the page it's
rendering in — not other pages. See [Macros](macros.md) and
[the build pipeline](../concepts/build-pipeline.md#consequences-worth-knowing).

## See also

- [Frontmatter reference](../reference/frontmatter.md)
- [Drafts](drafts.md) — work-in-progress pages
- [Permalinks](permalinks.md) — controlling output URLs
