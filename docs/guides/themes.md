# Themes

A theme bundles templates, archives, static assets, and config defaults in a
folder, so a whole look-and-feel can be shared and reused.

## Using a theme

Point at a theme directory with the top-level `theme:` key:

```yaml
# config.yaml
theme: themes/obsidian
```

Get starter themes by cloning the themes repo into your site:

```sh
git clone --depth 1 https://github.com/gordonbrander/italic_themes.git themes/
```

Then, optionally, copy the theme's starter content into your `content/`
directory (existing files are skipped):

```sh
italic scaffold
```

## How a theme layers

When a theme is set, italic overlays it **beneath** your site. Your files and
config win wherever both provide something; the theme fills in everything you
don't.

- **Templates, archives, and static assets** — the theme's `templates/`,
  `archives/`, and `static/` form the base layer. A file in your site's
  directory with the same relative path overrides the theme's; anything you
  don't provide falls through to the theme. So you can adopt a theme wholesale
  and override just one partial or stylesheet.
- **Config** — the theme's `config.yaml` provides defaults your site
  overrides: `collections` and `defaults` merge by name (your entry replaces a
  same-named theme entry; the theme's other entries are kept), `taxonomies`
  are unioned, and the `site:` map is deep-merged with your values winning.
  `hashtags` is on if either side enables it; `related.weights` is yours
  wholesale if you set any, otherwise the theme's.
- **`data/`, `content/`, and the output directory stay yours** — a theme never
  ships data or content, nor dictates where your content lives or output goes.

A theme always uses the conventional `templates/`, `archives/`, and `static/`
subdir names relative to its root; `*_dir` keys in a theme's own `config.yaml`
do not apply to it. A theme without a `config.yaml` still contributes its
files. Themes don't nest: a `theme:` key inside a theme's own config is
ignored.

## Authoring a theme

A theme is just a folder laid out like a site:

```
themes/my-theme/
  config.yaml     # config defaults the theme provides (optional)
  templates/      # Tera layouts, partials, macros
  archives/       # collection/taxonomy archive pages, feeds
  static/         # stylesheets, fonts, scripts
  content/        # starter content, copied by `italic scaffold` (optional)
```

Tips:

- Declare the collections and taxonomies your templates assume
  (`posts`, `tags`, …) in the theme's `config.yaml`, with `defaults:` wiring
  members to your layouts. Sites can override any of it by name.
- Reference assets through the URL filters (`relative_url`) so themed sites
  work under a `base_path`.
- Ship demo content in `content/` so `italic scaffold` gives users a working
  starting point.

Themes live outside your project history — any directory with this layout
works, referenced by path. `italic new` ships no theme; bring your own or
point at a shared one.

## See also

- [Configuration reference: theme merging](../reference/config.md#theme-config-merging)
- [CLI reference: scaffold](../reference/cli.md#italic-scaffold)
- [Project layout](../concepts/project-layout.md)
