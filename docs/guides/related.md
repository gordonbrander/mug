# Related pages

Italic can surface the pages most related to a given page — the heart of a
digital garden. It works with zero configuration and gets sharper as you add
tags and links.

## How relatedness is computed

Relatedness is **weighted shared-term overlap**: two pages are related in
proportion to how much they have in common, across two kinds of namespace:

- **Taxonomies** — pages that share terms (two notes tagged `phenomenology`).
- **`links`** — the whole wikilink graph, in both directions. Broader than the
  `backlinks` filter (incoming links only): a single symmetric measure relates
  two pages when *any* of these hold —
  - one page links to the other (an outbound link), *or*
  - one page is linked to by the other (a backlink), *or*
  - both pages link to the same third page (a shared reference).

  Because it's symmetric, if it relates A to B it also relates B to A.

## Configuring weights

Each namespace carries a weight set under `related:` in `config.yaml`, so you
decide whether a shared tag counts for more or less than a shared link:

```yaml
related:
  weights:
    tags: 2.0      # a taxonomy: shared tags
    series: 1.0    # any declared taxonomy can be weighted
    links: 1.0     # the whole link graph (both directions)
```

`weights` is the only key — the entire `related:` block is optional. With no
block, every declared taxonomy and the `links` graph get equal weight, so it
works zero-config: relating by `links` from day one, and by `tags` (and any
other taxonomy) once you declare it. Leaving a stale `limit:` in the block is
an error pointing you to the filter argument that replaced it.

## Rendering related pages

Use the `related` filter in a layout:

```jinja
<h2>Related</h2>
<ul>
{% for doc in page.id_path | related(limit=5) %}
  <li><a href="{{ doc.id_path | link }}">{{ doc.title }}</a></li>
{% endfor %}
</ul>
```

Results are ranked best-match first; a page is never related to itself; ties
break by date (newest first) then `id_path`. `limit` and `omit` are per-call
filter arguments — see the
[template reference](../reference/templates.md#related--pages-related-to-this-page).

## Tuning tips

- Weight taxonomies you curate deliberately (a hand-assigned `series`) above
  high-volume ones (free-form `tags`).
- If hashtags are enabled, casual inline `#tags` flow into the `tags`
  namespace too — lower its weight if that adds noise.
- The `links` namespace rewards densely interlinked notes; in a sparsely
  linked vault, taxonomy weights will do most of the work.

## See also

- [Wikilinks & backlinks](wikilinks.md) — where the link graph comes from
- [Taxonomies & hashtags](taxonomies.md)
- [Configuration reference: related](../reference/config.md#related)
