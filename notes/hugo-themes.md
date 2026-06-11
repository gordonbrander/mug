# How Hugo's themes work

Hugo's theme system is essentially a **layered/union filesystem**. Understanding
that mental model is the key.

## The core idea: themes are an overlay, project files win

When you set a theme, Hugo doesn't "switch" to the theme's files — it **merges**
the theme's directories with your project's root directories, with **your project
files taking precedence** on any conflict.

A theme is just a project-shaped bundle of the same directories your site has:

```
themes/my-theme/
├── layouts/        # templates
├── static/         # static assets
├── assets/         # files processed by Hugo Pipes
├── content/        # example/default content
├── data/
├── i18n/
├── archetypes/     # content scaffolding
└── theme.toml      # theme metadata
```

For each of these, Hugo builds a **merged virtual filesystem**: it looks in your
project root first, then falls back to the theme. If you have `layouts/index.html`
in both your project and the theme, **your project's wins**. This is why you
"override" a theme by copying a file to the same relative path in your project
root and editing it — no theme files are ever modified.

## Configuration

```toml
# config.toml
theme = "my-theme"
```

The theme name must match a directory under `themes/`. Themes are typically
vendored as git submodules or installed via Hugo Modules.

## Template lookup vs. the union filesystem

It's worth separating two mechanisms that both feel like "where does Hugo find
the template":

1. **The union filesystem** (above) — determines, for a *given path* like
   `layouts/_default/single.html`, whether the project or theme copy is used.
2. **The lookup order** — Hugo's rules for *which path* it wants for a given
   page, from most specific to least specific. For a single page it tries things
   like:
   - `layouts/<section>/<kind>.html`
   - `layouts/_default/single.html`
   - ... falling back to `_default/`

These compose: Hugo computes the ordered list of candidate paths via the lookup
order, then resolves *each candidate* against the union filesystem
(project-then-theme). The first candidate that exists *anywhere* (project or
theme) wins. So a project's `_default/single.html` will be used over a theme's
`_default/single.html`, but a theme's more-specific `blog/single.html` could
still be picked over a project's generic `_default/single.html`, because the
lookup order ranks `blog/single.html` higher before the filesystem is even
consulted.

## Multiple themes / theme components

`theme` can be an **array**, which turns themes into composable *components*:

```toml
theme = ["my-theme", "base-theme", "seo-component"]
```

Precedence runs left to right, with your project root still on top of all of them:

```
project root  >  my-theme  >  base-theme  >  seo-component
```

This lets a theme depend on other themes (Hugo reads each theme's own
`theme.toml` for its `[module]` imports). A "component" might ship only a `data/`
file or a partial, not a full template set — because everything merges, partial
components compose cleanly.

## Config merging

Theme configs are also merged, but more conservatively. By default the theme's
config is **mostly ignored** except for specific mergeable keys (params, menus,
etc.), and even then your project's values win. Hugo's `_merge` strategy controls
this. The intent: a theme can supply default `params`, but the site author always
overrides.

## The "default templates dir" (Hugo's embedded templates)

Hugo ships a handful of internal templates (RSS, sitemap, some shortcodes, Google
Analytics, etc.) compiled into the binary. These sit at the **bottom** of the
precedence stack, below all themes:

```
project root  >  themes...  >  Hugo's embedded templates
```

So you can override the built-in RSS template by placing your own at the path
Hugo looks for it (`_default/rss.xml`) in either your project or a theme.

## The one rule

The whole thing reduces to one rule:

> For any given relative path, search project → theme(s) left-to-right → Hugo
> built-ins, first hit wins.

Everything else (overriding, components, config merge) falls out of that.
