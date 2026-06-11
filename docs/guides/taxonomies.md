# Taxonomies & hashtags

Taxonomies categorize documents. Tags are the familiar example, but italic has
no built-ins — any frontmatter field can become a taxonomy: category, series,
publication, phase of the moon.

## Declaring taxonomies

List the frontmatter fields to treat as taxonomies in `config.yaml`:

```yaml
taxonomies:
  - tags
  - category
  - series
```

Then assign terms in any document's frontmatter:

```yaml
---
title: Building a second brain
tags: [pkm, writing]
series: [garden-notes]
---
```

A document's memberships are available in templates as `page.terms` — a map of
taxonomy → term slug → display text (e.g. `page.terms.tags`).

## Hashtags

With `hashtags: true` in `config.yaml`, italic lifts inline `#hashtags` out of
Markdown bodies into the `tags` taxonomy and strips them from the rendered
HTML:

```markdown
Quick thought about composting ideas. #pkm #gardening
```

…tags the page `pkm` and `gardening`, with the hashtags gone from the output.
This happens during markup, so hashtag-derived terms count everywhere
frontmatter tags do: tag archives, `taxonomy()`, and
[related-page](related.md) scoring. It's off by default so literal `#`
characters in prose are untouched.

## Using taxonomies in templates

List a taxonomy's terms and their documents with `taxonomy()`:

```jinja
{% for slug, docs in taxonomy(name="tags") %}
  <h2>{{ slug }}</h2>
  {% for post in docs %}
    <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
  {% endfor %}
{% endfor %}
```

For a deterministic order, pipe through `entries`:

```jinja
{% for entry in taxonomy(name="tags") | entries %}
  {{ entry.key }} ({{ entry.value | length }})
{% endfor %}
```

## Term archive pages

Generate one page per term — `/tags/rust/`, `/tags/tools/` — with a taxonomy
archive. The `:term` permalink variable is the term's slug:

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

See [Archives, feeds & sitemaps](archives.md).

## See also

- [Configuration reference: taxonomies](../reference/config.md#taxonomies)
- [Related pages](related.md) — taxonomies as relatedness signals
- [Template reference: taxonomy()](../reference/templates.md#taxonomyname--list-a-taxonomys-terms)
