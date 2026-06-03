# Static Site Generator — Design Spec

The static site generator for creatives.

A minimal, convention-light static site generator written in **Rust**. The
guiding principle is that *all* customization lives in **config, frontmatter,
and templates** — never in code or scripts. The CLI is opinionated, has
sensible defaults, and needs zero config to do something useful.

Audience: artists and creatives.
Unique features: digital gardens and portfolios.

---

## 1. Goals

- **Zero-config CLI.** Running the tool in a project root Just Works with no
  flags. There is one command to build and one to watch.
- **Customization is data, not code.** Everything that varies between sites is
  expressed in `config.yaml`, per-document frontmatter, and Tera templates.
  Because Rust is compiled, we deliberately avoid any embedded scripting layer.
- **Few mechanisms, composed.** Source content is classified once into named
  **collections** (saved queries) and **taxonomies** (named classifications such
  as `tags`), frozen, and then read by templates and by **archives** — files in
  `archives/` that fan a collection or taxonomy out into list/term pages. RSS
  feeds and tag archives are just archives; a sitemap is an ordinary content page
  that iterates a collection. No bespoke per-feature subsystems.
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
archives/       # Templates whose frontmatter declares a kind (collection|taxonomy) → fan out into archive pages.
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
| `terms`       | Term memberships: `taxonomy → (slug → display-text)`. Uplifted from each taxonomy's frontmatter field (and inline `#hashtag`s feed the built-in `tags` taxonomy when enabled); defaults to `{}`. See §5.2 and §7.1. |
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
iterate `{% for slug, text in page.terms.tags %}`. Listing docs by tag is done
through the `tags` taxonomy — `taxonomy(name="tags")` and `kind: taxonomy`
archives (§7.1) — not a query filter.

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

### Phase 2 — `classify` (collections)

Evaluate every `collections:` query and cache its membership (§9). Collection
membership is pure frontmatter metadata, so it is available before any body is
rendered. Running it here lets the next phase target a collection's members.

### Phase 3 — `defaults`

Apply per-collection default frontmatter from the `defaults:` config block. Each `defaults:` entry names a
collection; its values fill keys the collection's members did not set themselves
(the document's own frontmatter always wins). Because defaults land before
markup, a defaulted taxonomy field (e.g. `tags`) is seen by taxonomy
classification, and a defaulted `permalink`/`date` is reflected in `output_path`.

### Phase 4 — `markup`

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

### Phase 5 — `classify` (taxonomies)

Bucket every taxonomy's terms (§9), reading each doc's `terms` (now final after
the markup `#hashtag` pass). With collections (Phase 2) already cached, the index
is now fully classified and is **frozen** for the remaining phases. Only source
content is classified — archive-generated pages added in Phase 6 are deliberately
absent, so `collection()`/`taxonomy()` always list authored content.

### Phase 6 — `archives` (+ markup)

Expand each file in `archives/` into zero or more **view pages** over docs that
already exist. An archive's frontmatter declares a `kind`:

- **`collection`** — paginate the named collection into list pages.
- **`taxonomy`** — emit one (optionally paginated) archive page per term.

The page-1 URL is the archive's `permalink` verbatim; pages ≥2 append a
`page/N/` segment. Emitted pages are markup-processed like authored content and
**join the live index** for rendering/writing, but are *not* re-classified.
Because each archive reads only the frozen classification (never another
archive's output), archives are mutually independent and run in parallel — there
is no `order`/`weight` key.

### Phase 7 — `template`

Render Tera over each document's `content` using a context composed of:

- the document and its `data` (plus `pagination`/`term` for archive pages),
- site-wide data (`config.yaml` + the `data/` cascade).

In this phase the **full function set is available**, including the
classification-backed `collection()`/`taxonomy()` functions and the `backlinks`
filter. The template phase is uniform: authored pages and archive pages are
templated identically — nothing downstream needs to know which is which.

---

## 7. The Index

A single **in-memory** doc index is the spine of the system, holding all source
docs keyed by `id_path`. Derived listings are computed during classification and
then frozen:

- **collections** — named saved-query results (§9), computed before markup,
- **taxonomies** — named classifications (`tags` plus any declared), each a
  `term → docs` map, computed after markup,
- **backlinks** — the inverted wikilink graph (§8).

The index is populated during `read`, classified in two halves (collections
before markup, taxonomies after), and then **frozen**: archives and the template
phase read it by shared reference and emit their output to the side, so the index
is never mutated after classification and there is no corpus-wide clone.

### 7.1 Taxonomies

A **taxonomy** is a named classification of docs. The built-in `tags` taxonomy is
always present; declare more under `taxonomies:` in `config.yaml`:

```yaml
taxonomies:
  categories:          # field defaults to the taxonomy name
  series:
    field: serie       # override the frontmatter field
```

The built-in `tags` is merged in, not replaced (opt out with `tags: false`). Each
doc lists its terms under the taxonomy's frontmatter field; they are uplifted into
`doc.terms` (`taxonomy → slug → display text`). Templates read a whole taxonomy
with `taxonomy(name="tags")` (`term → docs`), and `kind: taxonomy` archives emit
a page per term.

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

The same query semantics drive both the `collections:` definitions in
`config.yaml` and (indirectly, via named collections) the archives, so authors
learn one model.

Capabilities:

- **Filter by path** — prefix or, preferably, **glob** (`pages/*.md`).
- **Order by** `title`, `date`, or `updated`, with sort direction.
- **Limit** the number of results.

Filtering by term is the job of taxonomies (§7.1), not queries — a collection
query is pure path-glob + ordering, which keeps it independent of the markup
phase (so collections classify before markup; see §6).

**Optional compact query language** (under consideration):

```
path:pages/*.md order_by:updated sort:desc limit:10
```

### 9.1 Archive specifics

An **archive** (file in `archives/`) declares a `kind` and names a classification:

- **`kind: collection`** + **`collection:`** — paginate the named collection.
- **`kind: taxonomy`** + **`taxonomy:`** — one run per term of the named taxonomy;
  `:term` in the `permalink` is the term slug, and the body/template receives a
  `term` (`slug`, `text`) context.
- **`per_page`** — items per output page; defaults to infinity (single page).
  Each page receives a `pagination` context (current page, total pages, prev/next
  URLs, item slice).
- **`permalink`** is the page-1 (landing) URL; pages ≥2 append `page/N/`.

Archives never observe one another's output (only the frozen classification of
source content), so there is no execution-order key — they run in parallel.

---

## 10. Built-in Archives (Scaffolded)

These are not special-cased subsystems — they are ordinary files the tool can
**scaffold** for the user:

- **RSS** — a `kind: collection` archive over a `recent` collection, emitting a
  single feed document.
- **Sitemap** — an ordinary *content* page (`content/sitemap.html`) that iterates
  a collection via `collection()`. It is content, not an archive, because a
  sitemap lists canonical pages, not paginated archive pages.

Because they are just files in `archives/` and `content/`, users may edit or
delete them freely. The "automatic" part is purely scaffolding convenience.

---

## 11. Known Tensions & Deliberate Lines

These are explicit design decisions, recorded so they are chosen rather than
stumbled into:

1. **Classification vs. incremental builds.** Any page may list any other via
   `collection()`/`taxonomy()`. This makes correct incremental rebuilding hard.
   v1 is full-rebuild only. *If* incrementality is added later, each page's
   classification inputs must be logged so invalidation can flow from them —
   cheap to anticipate now, expensive to retrofit.

4. **Generated pages are excluded from classification.** Archives read the frozen
   classification of *source* content and append view pages that are never
   re-classified. This makes archives order-independent and parallel by
   construction, and avoids the feedback-loop hazards that other SSGs hit when
   generated pages re-enter the collections that drive generation. A whole-site
   listing (e.g. a sitemap) is therefore an authored content page, not an
   archive.

2. **Macros expand before markup render.** Tera macros run *before*
   Markdown rendering, producing a flattened string that can be rendered once.
   This keeps the "fully render before templating" model coherent and avoids the
   deferred-rendering circularity that more complex generators have hit.
   Given the Markdown-and-raw-HTML-only scope, this is a comfortable line to hold.

3. **Restricted markup-phase Tera.** Classification-backed functions
   (`collection`, `taxonomy`, `backlinks`) are unavailable during `markup` and
   available during `template`. This enforces the classify-before-listing
   invariant by construction rather than by discipline.

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
- `new` / scaffolding — generate a starter RSS archive and sitemap page, etc.

All behavioral configuration lives in files, not flags.
