# Data files

The `data/` directory holds YAML files that every template can read — site
navigation, author bios, link rolls, anything that's structured data rather
than a page.

## How it works

Each top-level YAML file in `data/` is loaded once per build and surfaced to
all templates (and document bodies) as `{{ data.<stem> }}`, keyed by filename
stem.

```yaml
# data/nav.yaml
- title: Home
  path: /
- title: Garden
  path: /garden/
- title: About
  path: /about/
```

```jinja
<nav>
  {% for item in data.nav %}
    <a href="{{ item.path | relative_url }}">{{ item.title }}</a>
  {% endfor %}
</nav>
```

A mapping file works the same way:

```yaml
# data/authors.yaml
gordon:
  name: Gordon Brander
  url: https://gordonbrander.com
```

```jinja
{% set author = data.authors[page.data.author] %}
<address>{{ author.name }}</address>
```

## `data:` vs `site:`

Both are global, so which to use? `site:` (in `config.yaml`) suits a handful
of scalar settings — title, description, URL — and is what themes deep-merge
their defaults into. `data/` suits anything bigger or structured: lists, maps,
content-like data you'd rather keep out of config. Files keep concerns
separate and diff cleanly.

Note that `data/` is always yours: themes never ship data files.

## See also

- [Configuration reference: site](../reference/config.md#site)
- [Template reference: context](../reference/templates.md#context)
