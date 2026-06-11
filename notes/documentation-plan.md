# Documentation plan

## Goals

Italic's documentation should get a writer from "folder full of Markdown" to a
published digital garden with as little ceremony as possible, while still giving
theme authors and power users an exhaustive reference. Concretely:

- **Fast time-to-first-site.** A newcomer should reach a rendered site in under
  five minutes without reading anything but the quickstart.
- **Garden-first framing.** Wikilinks, backlinks, related pages, and Obsidian
  compatibility are the differentiators — they get top billing, not a footnote
  after generic SSG features.
- **Diátaxis structure.** Separate tutorials (learning), guides (doing),
  reference (looking up), and explanation (understanding) so each page has one
  job and a predictable shape.
- **Reference that can't drift.** Every CLI flag, config key, frontmatter key,
  and template filter/function documented with type, default, and a working
  example — ideally generated or test-verified against the code.
- **Unbundle the README.** The README currently carries the entire manual.
  Migrate the depth into dedicated pages and slim the README down to pitch,
  install, quickstart, and links.

Organization follows [docs-outline.md](docs-outline.md). Each
checklist item below is one documentation page; sub-bullets are the content
that page should cover.

## Checklist

### Getting started

- [x] **Installation** (`getting-started/installation.md`)
  - `cargo install italic`; where the binary lands (`~/.cargo/bin`)
  - Prebuilt binaries / Homebrew (when available; note as future work)
  - Building from a clone of the repo
  - Verifying the install (`italic --version`), upgrading, uninstalling
- [x] **Quickstart** (`getting-started/quickstart.md`)
  - `italic new my-site` → add a page → `italic serve` → view at localhost:3000
  - Installing a starter theme from `italic_themes` and setting `theme:` in `config.yaml`
  - `italic scaffold` to pull in demo content
  - `italic build` and where output goes (`public/`)
  - Pointers to the tutorial and core concepts for next steps
- [x] **Tutorial: publish your Obsidian vault** (`getting-started/tutorial.md`)
  - Walkthrough: point italic at an existing vault (or folder of Markdown)
  - Wikilinks and backlinks working out of the box; hashtags → tags
  - Add frontmatter, a base template, and a posts collection
  - Add a tag archive and an RSS feed
  - Deploy the result (link out to the deployment guide)

### Core concepts

- [x] **Project layout** (`concepts/project-layout.md`)
  - `content/`, `templates/`, `archives/`, `data/`, `static/`, `themes/`, `config.yaml`
  - "Italic doesn't impose a content layout" — collections define structure, not folders
  - Which directories are configurable via `*_dir` keys
- [x] **Content model** (`concepts/content-model.md`)
  - The three content types: `.md`, `.html`, `.yaml` and how each renders
  - `id_path` as the canonical document identity used throughout templates
  - How permalinks mirror source paths by default
  - Where frontmatter lands (`page.*` special keys vs. `page.data.*`)
- [x] **The build pipeline** (`concepts/build-pipeline.md`)
  - Phases: content (Tera pre-render) → markup → template → archives → static copy
  - What context is available in which phase (and why wikilinks/`markdown` filter
    differ between phases)
  - Drafts dropped at the start of the build; what that implies for collections,
    taxonomies, backlinks
  - Parallel rendering; archives are order-independent by design

### Guides

- [x] **Authoring content** (`guides/authoring.md`)
  - GitHub-flavored + Obsidian Markdown support; syntax-highlighted code fences
  - Frontmatter: the special keys table (`title`, `draft`, `template`, `tags`,
    `date`, `updated`, `permalink`) with defaults
  - Arbitrary keys on `page.data`; term memberships on `page.terms`
- [x] **Wikilinks & backlinks** (`guides/wikilinks.md`)
  - `[[Page Title]]` and `[[Page Title|Display text]]` syntax
  - The Obsidian-style fuzzy resolution algorithm (current dir first, expanding)
  - Rendered markup: `a.wikilink` vs `span.nolink`; styling unresolved links
  - How resolved links feed the backlink graph; the `backlinks` filter
- [x] **Related pages** (`guides/related.md`)
  - What "related" means: weighted shared-term overlap across taxonomies + the
    symmetric `links` graph (outbound, backlink, co-citation)
  - Configuring `related: weights:` in `config.yaml`; zero-config defaults
  - Using the `related` filter in templates; ranking and tie-breaking
- [x] **Collections** (`guides/collections.md`)
  - Collections as saved queries: `path`, `order_by`, `sort`, `omit`
  - Multiple blogs/sections/portals from one site
  - Reading collections in templates with `collection(name=...)`
  - Collection `defaults:` (permalink + template per collection; precedence rules)
- [x] **Taxonomies & hashtags** (`guides/taxonomies.md`)
  - Declaring taxonomies in `config.yaml`; any frontmatter field as a taxonomy
  - `hashtags: true` — inline `#hashtags` lifted into `tags` and stripped
  - Reading taxonomies in templates with `taxonomy(name=...)`; term archives
- [x] **Templates** (`guides/templates.md`)
  - Tera basics and link-out; supported extensions (`.html`, `.xml`, `.tera`,
    `.json`, `.txt`) and autoescaping rules per extension
  - The template context: `page`, `site`, `data`, plus `pagination`/`term` on archives
  - Inheritance, partials, a worked `base.html` example
- [x] **Macros (shortcodes) & content templates** (`guides/macros.md`)
  - Macro files in `templates/macros/`, auto-import in the content phase
  - Calling macros from Markdown (video embeds, responsive images)
  - The content-phase Tera render: what works in a doc body, what doesn't
    (no cross-page data in the content phase)
- [x] **Archives, feeds & sitemaps** (`guides/archives.md`)
  - Archive templates in `archives/`; `kind: collection` vs `kind: taxonomy`
  - Frontmatter keys: `collection`/`taxonomy`, `permalink` (with `:term`),
    `per_page`, `limit`, `template`; how `limit` and `per_page` compose
  - The `pagination` context and prev/next navigation pattern
  - Recipes: RSS/Atom feed, sitemap, JSON feed via `.json` templates, robots.txt
- [x] **Permalinks** (`guides/permalinks.md`)
  - Default path mirroring; overriding via frontmatter or collection defaults
  - Variables: `:slug`, `:yyyy`, `:mm`, `:dd`, `:term`; trailing `/` → `index.html`
  - `site.url` and `base_path`; the four URL filters and when to use each
- [x] **Drafts** (`guides/drafts.md`)
  - `draft: true`; drafts invisible to builds, collections, taxonomies, backlinks
  - `serve`/`watch` include drafts; `build --drafts` for staging previews
- [x] **Themes** (`guides/themes.md`)
  - Using a theme: `theme:` key, installing from a repo, `italic scaffold`
  - Layering rules: templates/archives come from the theme; config defaults
    merge (collections/defaults by name, taxonomies unioned, `site:` deep-merged);
    static overlay with site files winning
  - What stays yours: `data/`, `content/`, output dir
  - Authoring a theme: required layout, shipping starter content, no nesting
- [x] **Data files** (`guides/data.md`)
  - YAML files in `data/`, keyed by filename stem, reachable as `{{ data.* }}`
  - Use cases: navigation menus, author lists, site-wide settings beyond `site:`
- [x] **Deployment** (`guides/deployment.md`)
  - `italic build` output is plain static files; `clean` before fresh builds
  - Recipes: GitHub Pages (with `base_path`), Netlify, Cloudflare Pages, rsync
  - CI example: build-and-deploy workflow
- [x] **Migrating to italic** (`guides/migration.md`)
  - From Obsidian Publish / Quartz: what carries over directly
  - From Jekyll/Hugo/Zola: frontmatter mapping, permalink patterns, template
    syntax differences (Tera vs Liquid/Go templates)

### Reference

- [x] **CLI reference** (`reference/cli.md`)
  - Every command: `build` (`--drafts`), `serve`, `watch`, `new <path>`,
    `scaffold`, `clean` — flags, defaults (port 3000, `public/`), exit behavior
- [x] **Configuration reference** (`reference/config.md`)
  - Every `config.yaml` key with type, default, example: `*_dir` keys, `theme`,
    `site:` (incl. `url`, `base_path`), `collections:`, `taxonomies:`,
    `related:`, `defaults:`, `hashtags`
- [x] **Frontmatter reference** (`reference/frontmatter.md`)
  - All special keys with types and default-derivation rules (e.g. `date` from
    file created → modified time)
  - Archive frontmatter keys (`kind`, `collection`, `taxonomy`, `per_page`, `limit`)
- [x] **Template reference** (`reference/templates.md`)
  - Full context shape: `page`, `site`, `data`, `pagination`, `term`
  - Every italic function/filter with kwargs, phase availability, and example:
    `collection()`, `all()`, `taxonomy()`, `doc()`, `dir()`, `backlinks`,
    `related`, `entries`, `dirtree`, `filter_in_dir`, `omit_docs`,
    `truncate_words`, `markdown`, `permalink`, `link`, `relative_url`,
    `absolute_url`
  - Link to Tera built-ins rather than duplicating them

### Meta

- [x] **Comparison: why italic?** (`comparison.md`)
  - Honest positioning vs. Hugo, Zola, Eleventy, Quartz, Obsidian Publish
  - What italic doesn't do (asset pipeline, i18n, …) so expectations are set
- [x] **Troubleshooting / FAQ** (`troubleshooting.md`)
  - Unresolved wikilinks, template errors and how Tera reports them, permalink
    collisions, drafts unexpectedly missing/present, `base_path` pitfalls
- [x] **Changelog & upgrade notes** (`CHANGELOG.md`)
  - Keep-a-changelog format; call out breaking config/template changes per release
- [x] **Contributing & architecture** (`contributing.md`)
  - Repo layout, how to run tests, a short tour of the rendering pipeline
    internals for new contributors
- [x] **README slimming** (`README.md`)
  - Once the pages above exist, trim the README to: pitch, feature highlights,
    install, quickstart, and a docs table of contents
