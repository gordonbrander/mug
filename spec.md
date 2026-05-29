# Static Site Generator — Design Spec

A minimal, convention-light static site generator written in **Rust**. The
guiding principle is that *all* customization lives in **config, frontmatter,
and templates** — never in code or scripts. The CLI is opinionated, has
sensible defaults, and needs zero config to do something useful.

---

## 1. Goals

- **Zero-config CLI.** Running the tool in a project root Just Works with no
  flags. There is one command to build and one to watch.
- **Customization is data, not code.** Everything that varies between sites is
  expressed in `config.yaml`, per-document frontmatter, and Tera templates.
  Because Rust is compiled, we deliberately avoid any embedded scripting layer.
- **One mechanism, not many.** There is no special-cased notion of "section,"
  "collection,". Collections are produced on demand by *querying*
  and *globbing* over a single in-memory index during the generate/template
  phases. RSS feeds, sitemaps, and tag archives are all just *generated pages*,
  not bespoke subsystems.
- **A small, predictable surface area.** Only three input formats (Markdown,
  raw HTML, YAML), one template engine (Tera), and a fully-rendered in-memory
  corpus that downstream phases can rely on.

---

## 2. Non-Goals (v1)

- **Incremental builds.** v1 is full-rebuild only. (The query model is
  fundamentally at odds with cheap incremental builds; see §11.) The design
  leaves room to add this later but does not pay for it now.
- **Asset pipeline.** No bundling, minification, or image processing. `static/`
  is copied verbatim. Fingerprinting/cache-busting is out of scope for v1.
- **Pluggable markup.** Only Markdown and raw HTML. No reStructuredText, no
  AsciiDoc, no alternative renderers.
- **Embedded scripting / plugins.** Tera macros are the only extension point.

---

## 3. Project Layout

```
content/        # Authored documents (.md, .html, .yaml). Render to their own paths.
generators/     # Templates whose frontmatter describes a query → fan out into pages.
templates/      # Tera layouts/partials/macros referenced by documents.
data/           # YAML files mixed into the global data cascade.
static/         # Copied verbatim into the build output.
config.yaml     # The only config file. Optional; sensible defaults apply.
```

There is no `posts/` or `pages/` convention baked in. Subdirectories under
`content/` are merely path prefixes — meaningful only insofar as queries and
globs target them. A blog is `content/posts/*.md` *by convention of the author's
queries*, not by any built-in section concept.

---

## 4. Input Doc Types

Markdown and HTML carry **frontmatter** (a leading `---`-delimited YAML block).
YAML documents *are* their own frontmatter.

| Type    | Frontmatter        | Body / content                                  |
|---------|--------------------|-------------------------------------------------|
| `.md`   | Optional YAML block | Markdown, rendered to HTML                       |
| `.html` | Optional YAML block | Raw HTML, passed through                         |
| `.yaml` | The whole file      | `content` field rendered as HTML (see §6)        |

---

## 5. The Doc Model

Every input file becomes a `Doc`. Fields are *uplifted* from frontmatter
where noted, with defined fallbacks.

| Field         | Source / Default                                                                 |
|---------------|----------------------------------------------------------------------------------|
| `id_path`     | Original (or synthesized) path, relative to the source dir. Stable identity.     |
| `output_path` | Eventual render location, relative to the build dir. See §5.1.                   |
| `template`    | Uplifted from frontmatter (`template`).                                          |
| `title`       | Uplifted from frontmatter; defaults to `""`.                                     |
| `content`     | The rendered body (HTML after the markup phase). See §6.                         |
| `tags`        | Slug → display-text map. Uplifted from frontmatter (and inline `#hashtag`s when enabled); defaults to `{}`. See §5.2. |
| `date`        | `date` frontmatter → file created time → file updated time.                      |
| `updated`     | `updated` frontmatter → file updated time.                                       |
| `data`        | The verbatim frontmatter map (everything, including the uplifted keys).          |

### 5.1 `output_path` and the `permalink` field

By default a document renders to a location mirroring its `id_path`. A
`permalink` field in frontmatter overrides this with a path *template* expanded
against a fixed set of variables:

- `slug` — sluggified stem of the document
- `yyyy`, `mm`, `dd` — components of `date`
- (extensible, but kept deliberately small)

Example: `permalink: /blog/:yyyy/:slug/`.

`output_path` is computed **before** rendering, because the markup phase needs to
resolve `permalink`-based links (see §6.1).

### 5.2 Tags

`tags` is a map **keyed by the slugified tag text, with the display text as the
value** (e.g. `My Tag` → `my-tag: "My Tag"`). Keying by slug deduplicates tags
that slugify identically and gives templates both a URL-safe slug and the
original text; iteration is sorted by slug for deterministic output. Templates
iterate `{% for slug, text in page.tags %}`, and `query(tag=…)` matches by slug,
so `tag="My Tag"` and `tag="my-tag"` are equivalent (see §9).

Tags come from the frontmatter `tags:` sequence. Optionally — when
`hashtags: true` is set in `config.yaml` — the markup phase also scans Markdown
bodies for inline `#hashtag`s, adds them to `tags`, and **strips them from the
rendered output**. A hashtag is a `#` at a word boundary (start of text or after
whitespace) followed by `[A-Za-z0-9_-/]` (so slug paths like `#project/mug`
work) containing at least one letter (so `#123` is ignored). Because scanning
runs on the parsed Markdown AST, heading markers (`# Heading`) and code
spans/fences are never mistaken for tags. Frontmatter text wins on a slug
collision with an inline hashtag. The flag is off by default, leaving literal
`#` in prose untouched.

---

## 6. Rendering: The Phased Pipeline

The build runs in four ordered phases. The key invariant: **the global index is
fully populated before any index-dependent template logic runs**, so any page
can list or reference any other page.

```
read -> markup -> generate (+ markup) -> template
```

### Phase 1 — `read`

Walk `content/`, classify files, parse frontmatter, compute `id_path` and
`output_path`, and construct `Doc` structs. Bodies are *not* rendered yet.
Load `data/` into the data cascade. This phase populates the index keys (§7).

### Phase 2 — `markup`

Render each document's body to its final `content` (HTML). This phase runs Tera
**with a restricted configuration** — index-based filters such as `query` and
`backlinks` are *not* available, because the index is not yet meaningful for
cross-page listing at body-render time.

Per type:

- **Markdown** — process frontmatter → expand Tera macros → render Tera
  (restricted) → render Markdown.
- **HTML** — process frontmatter → render Tera (restricted) → done.
- **YAML** — render the `content` field through Tera (restricted); the result is
  assumed to be HTML → done.

Ordering note: **Tera macros (the shortcode equivalent) run before Markdown
rendering.** Macros are not permitted to emit content that requires a further
markup pass — this is the line that keeps the phase model clean (see §10).

#### 6.1 The `permalink` filter

Because `output_path` for every document is known after `read`, the markup phase
can expose a **`permalink` filter** that expands a document's `id_path` into its
`output_path`. This is how authored content links to other content without
hardcoding output URLs (and it is the resolution target for wikilinks; see §8).

### Phase 3 — `generate` (+ markup)

Expand each template in `generators/` into zero or more **virtual pages**. A
generator's frontmatter declares a query (§9), pagination, and an output-path
pattern; the generator fans out into concrete `Doc`-shaped descriptors,
each carrying its bound query results and pagination context. These descriptors
are markup-processed like authored content and **join the index**.

Generators carry an integer **`order`** key (§9.1) so that generators which must
observe the output of *other* generators (e.g. a sitemap) can run last.

### Phase 4 — `template`

Render Tera over each document's `content` using a context composed of:

- the document and its `data`,
- site-wide data (`config.yaml` + the `data/` cascade),
- any generated/pagination data bound to the page.

In this phase the **full filter set is available**, including the index-backed
`query` and `backlinks` filters. The template phase is uniform: authored pages
and generated pages are templated identically — nothing downstream needs to know
which is which.

---

## 7. The Index

A single **in-memory, mutable** index is the spine of the system. Docs are
indexed by:

- **path** — for prefix/glob lookups,
- **tag** — for tag-based collections,
- **backlink** — the inverted wikilink graph (§8).

The index is populated during `read`, augmented during `generate`, and queried
during `template`.

---

## 8. Wikilinks & Backlinks

Markdown supports and `[[Wiki Link]]` and `[[Wiki Link|Display text]]` syntax.

**Resolution (nearest page matching):** sluggify the wikilink target and look
for any page whose `id_path` stem-slug matches — across the **entire doc set**,
not just the source's ancestor chain. When more than one page matches, the
candidate at the shortest **directory distance** from the source wins (steps up
to the nearest common ancestor plus steps back down to the candidate). Ties at
equal distance are broken by the lexicographically smallest `id_path` so output
is deterministic. The resolved target is rendered through the `permalink`
mechanism so the emitted URL is correct.

**Path-prefix disambiguation:** an author can anchor a link at the vault root
with `[[dir/sub/Name]]`. The prefix's components are slugified individually and
must match the candidate's parent directory exactly (no suffix matching); a
prefix with no match resolves to nothing, rather than falling back to a bare
stem lookup. `[[/Name]]` (empty prefix) matches only root-level docs.

Implementation: we can simply layer this render pass before or after a standard Markdown render.
Since Wikilink syntax is not part of Markdown, the renderer won't touch it.

**Backlink indexing:** every resolved wikilink records an edge in the index, so
the reverse direction (which pages link *to* this one) is queryable. A
**`backlinks` filter** exposes this during the template phase, with ordering by
`title`, `date`, or `updated`.

---

## 9. Querying

The same query semantics drive both the **`query` template filter** and
**generator** definitions, so authors learn one model.

Capabilities:

- **Filter by path** — prefix or, preferably, **glob** (`pages/*.md`).
- **Filter by tag.**
- **Order by** `title`, `date`, or `updated`, with sort direction.

**Optional compact query language** (under consideration):

```
path:pages/*.md, tag:journal order_by:updated sort:desc
```

### 9.1 Generation specifics

- Generators reuse the query syntax/parameters above to build their collection.
- **`per_page`** — items per output page; defaults to infinity (single page).
  Fan-out by page produces the paginated descriptors, each receiving a
  pagination context (current page, total pages, prev/next URLs, item slice).
- **`weight`** (integer) — controls generator execution order within phase 3.
  Generators that must see other generators' output set a high value; e.g. a
  **sitemap** uses `weight: 9999` to run last and observe everything emitted
  before it. (Implementation note: this field was originally named `order`
  in this spec; renamed to `weight` to disambiguate from `query.order_by`.)

---

## 10. Built-in Generators (Scaffolded)

These are not special-cased subsystems — they are ordinary generator templates
that the tool can **scaffold** for the user:

- **RSS** — a generator querying recent content, emitting a feed document.
- **Sitemap** — a high-`order` generator listing all prior output.

Because they are just files in `generators/`, users may edit or delete them
freely. The "automatic" part is purely scaffolding convenience.

---

## 11. Known Tensions & Deliberate Lines

These are explicit design decisions, recorded so they are chosen rather than
stumbled into:

1. **`query()` vs. incremental builds.** Any page may depend on any other,
   invisibly, through a query. This makes correct incremental rebuilding hard.
   v1 is full-rebuild only. *If* incrementality is added later, each page's
   query inputs must be logged so invalidation can flow from them — cheap to
   anticipate now, expensive to retrofit.

2. **Macros expand before markup render.** Tera macros run *before*
   Markdown rendering, producing a flattened string that can be rendered once.
   This keeps the "fully render before templating" model coherent and avoids the
   deferred-rendering circularity that more complex generators have hit.
   Given the Markdown-and-raw-HTML-only scope, this is a comfortable line to hold.

3. **Restricted markup-phase Tera.** Index-backed filters (`query`,
   `backlinks`) are unavailable during `markup` and available during
   `template`. This enforces the index-before-listing invariant by construction
   rather than by discipline.

---

## 12. Technology Choices

- **Language:** Rust.
- **Template engine:** **Tera** (Jinja-like, runtime, Rust-native). A runtime
  engine is required because users supply their own templates — compile-time
  engines are unsuitable here. `query` and `backlinks` are registered as custom
  Tera filters/functions backed by the index.
- **Config:** `config.yaml`, optional, with zero-config defaults.

---

## 13. CLI Surface (Sketch)

- `build` — run the full pipeline once into the output directory.
- `watch` — optional watch-and-rebuild loop (full rebuild on change in v1).
- `new` / scaffolding — generate starter RSS and sitemap generators, etc.

All behavioral configuration lives in files, not flags.
