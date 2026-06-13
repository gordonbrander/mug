# Documentation outline

Sections expected in a well-documented static site generator, organized roughly
along the Diátaxis axes (tutorials, how-to guides, reference, explanation).

## Getting started

- **Installation** — package managers, prebuilt binaries, building from source.
- **Quickstart** — zero to a rendered site in under five minutes: scaffold a
  site, add a page, run the dev server, build.
- **Tutorial** — a longer guided walkthrough that builds a realistic small site
  and touches templates, content, and deployment along the way.

## Core concepts (explanation)

- **Directory structure** — what goes where (content, templates, static assets,
  output), and what's convention vs. configurable.
- **Content model** — how pages, posts/collections, sections, and taxonomies
  (tags, categories) work; how URLs are derived from file paths
  (permalinks/slugs).
- **Build pipeline** — the lifecycle from source files to output: parsing,
  rendering, asset processing. This is the section most SSGs skip and users
  most need when debugging.

## Guides (how-to)

- **Markdown & frontmatter** — supported syntax, extensions (footnotes,
  wikilinks, syntax highlighting), frontmatter fields and their effects.
- **Templating** — template language, available variables/context per page
  type, inheritance/partials, custom shortcodes or filters.
- **Assets** — CSS/JS handling, images, fingerprinting/cache busting if
  supported.
- **Common recipes** — RSS/Atom feeds, sitemaps, drafts, pagination, redirects,
  multilingual sites if applicable.
- **Deployment** — at least GitHub Pages, Netlify, Cloudflare Pages, and plain
  rsync-to-a-server.
- **Migration** — from Jekyll/Hugo/Eleventy/etc., even if brief; this is how
  most users arrive.

## Reference

- **CLI reference** — every command and flag (`build`, `serve`, `new`, watch
  mode, ports).
- **Configuration reference** — every config key with type, default, and an
  example. Generated from code if possible so it can't drift.
- **Template variable/function reference** — the full context available in
  templates.

## Meta

- **Changelog / upgrade guides** — especially breaking changes between
  versions.
- **Troubleshooting / FAQ** — common build errors, live-reload quirks, path
  issues.
- **Contributing & architecture notes** — for a smaller or newer project, a
  short internals overview earns contributors cheaply.

## Garden-specific (italic)

- **Garden features** — wikilinks/backlinks, note organization, publishing
  workflows from tools like Obsidian; the differentiator people show up for.
- **Comparison** — why italic instead of Hugo/Quartz/etc.; sets expectations
  honestly.
