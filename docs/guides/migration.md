# Migrating to italic

Italic reads plain Markdown with YAML frontmatter, so most migrations are
mostly a matter of pointing it at your existing files and mapping config.

## From an Obsidian vault

The happy path — italic is built for this. Copy (or symlink) your vault into
`content/` and build:

- `[[Wikilinks]]` and `[[Wikilinks|aliases]]` resolve with the same fuzzy
  matching algorithm Obsidian uses; backlinks come for free.
- Inline `#hashtags` lift into the `tags` taxonomy with `hashtags: true`.
- Notes without frontmatter are fine — `title` defaults to empty (set it, or
  derive headings from your H1s in the layout), dates fall back to file
  timestamps.

What doesn't carry over: Obsidian plugins, dataview queries, canvas files, and
embeds (`![[...]]`). Start with the
[tutorial](../getting-started/tutorial.md), which walks this exact path.

## From Jekyll

| Jekyll | Italic |
|--------|--------|
| `_posts/` with dated filenames | A `posts` collection; put the date in frontmatter (or keep it in the filename and set `permalink:` per file). |
| `_config.yml` | `config.yaml` — `site:` holds your metadata. |
| `permalink: /blog/:year/:title/` | `permalink: /blog/:yyyy/:slug/` in collection `defaults:`. |
| `layout: post` | `template: post.html`. |
| Liquid (`{{ page.title }}`, `{% for %}`) | Tera — nearly identical interpolation/block syntax; filters differ in spots ([Tera built-ins](https://keats.github.io/tera/docs/#built-ins)). |
| `_data/*.yml` | `data/*.yaml`, as `{{ data.* }}`. |
| `categories`/`tags` | Declare both under `taxonomies:`. |

## From Hugo

| Hugo | Italic |
|------|--------|
| `content/` sections | Keep the folder structure; define [collections](collections.md) by glob instead of section. |
| `hugo.toml` | `config.yaml`. |
| `[permalinks]` patterns | `permalink:` in collection `defaults:` (`:yyyy`, `:mm`, `:dd`, `:slug`). |
| Go templates (`{{ .Title }}`) | Tera (`{{ page.title }}`) — syntax differs substantially; layouts need rewriting. |
| `layouts/_default/list.html` | An [archive template](archives.md). |
| Taxonomies in config | Same idea: `taxonomies:` array. |
| Shortcodes | [Tera macros](macros.md). |

## From Zola

The closest relative — Zola also uses Tera, so templates mostly port directly.
Differences to mind:

- Context names differ: Zola's `page.permalink`/`section` model vs. italic's
  `page.id_path` + URL filters; there are no "sections" — use
  [collections](collections.md).
- Zola's `_index.md` section pages become [archives](archives.md).
- `taxonomies` move from per-page config syntax to a plain `taxonomies:` array
  plus frontmatter fields.

## General checklist

1. Copy content into `content/`; don't restructure yet.
2. Declare your taxonomies, then your collections (globs over the existing
   layout).
3. Recreate permalinks with `defaults:` so URLs don't break; spot-check old
   URLs against the new output.
4. Port layouts to Tera one at a time, starting with `base.html`.
5. Wire archives for listings and feeds.

## See also

- [Tutorial: publish your Obsidian vault](../getting-started/tutorial.md)
- [Collections](collections.md) · [Permalinks](permalinks.md) · [Templates](templates.md)
