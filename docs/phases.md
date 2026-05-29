# Build Phases

The build runs as a linear pipeline. Each phase has a defined input shape, a
defined output shape, and a narrowed view of the index — the data available to
that phase is exactly what it needs and nothing more. This document is the
reference for what each phase can see and do.

---

## Overview

| # | Phase    | Reads                       | Writes                          |
|---|----------|-----------------------------|---------------------------------|
| 1 | load     | filesystem (`content/`)     | `Vec<Doc>` with raw bodies      |
| 2 | markup   | `Arc<Vec<Meta>>` snapshot   | `doc.content` (rendered HTML), `doc.links` |
| 3 | generate | `Arc<Vec<Doc>>` snapshot    | appended `Doc`s (pre-rendered)  |
| 4 | template | `Arc<Vec<Doc>>` snapshot    | final HTML for each doc         |
| 5 | write    | rendered docs               | filesystem (`build/`)           |

The key invariant: **a phase's snapshot is frozen at the moment the phase
starts.** Mutations to per-doc fields happen in place during the phase, but the
shared snapshot view is immutable for the phase's duration.

---

## 1. Load

Walks `content/`, parses frontmatter, and builds the initial `Vec<Doc>`.
Bodies are read into memory as raw source (Markdown, HTML, or YAML), not yet
rendered.

**Data access:** filesystem only. No index exists yet.

**Output:** `Index` containing every source doc with `id_path`, `output_path`,
frontmatter merged into `doc.data`, and `content` set to the raw body string.

---

## 2. Markup

Renders each doc's body: restricted Tera pass → comrak parse (Markdown only,
with the wikilink extension) → wikilink resolution on the AST → comrak HTML
render. Because wikilinks are resolved post-parse, `[[…]]` inside code spans
stays literal. See `src/build/markup.rs` and `src/build/markup/wikilink.rs`.

**Data access:** `Arc<Vec<Meta>>` — a projection of the index containing only
the fields needed to render cross-doc references (`id_path`, `output_path`,
`title`, `tags`, `date`, `updated`). **The full body content of other docs is
not visible** in this phase, by construction.

Why `Meta` and not `Doc`:

- During markup, every other doc's `content` is stale — it's still raw
  Markdown, mid-render, or already rendered depending on iteration order.
  Reading it would be a footgun.
- Narrowing to `Meta` makes that staleness invisible: code that *can't* see
  content can't accidentally depend on it.
- It's a type-level enforcement of spec §11's "no index-listing filters in
  markup" rule.

**Why `Arc`:** the markup-phase Tera env registers filter closures
(`permalink`, `link`) that capture the snapshot. Tera filters are
`Send + Sync + 'static`, so the closures need owned data, not a borrow. The
outer loop also needs `&mut index.docs` for write-back, which would conflict
with any immutable borrow from `index.docs`. `Arc<Vec<Meta>>` resolves both
constraints with a single allocation shared cheaply across closures and the
render loop. (With `Meta` instead of `Doc`, the cost difference between `Arc`
and plain clone-per-closure is small — `Arc` wins on style and "free future
filters," not on raw bytes.)

**Filters/functions available:** `permalink`, `link`, `relative_url`,
`absolute_url`, the `markdown` filter (see Generate). Macros from
`templates/macros/*.html` are auto-imported. `query` and `backlinks` are
**not** registered — the index is not meaningful for listing yet.

**Links population:** `wikilink::expand` returns `(expanded_string,
links)` and `markup::render` writes the links back onto the doc. This
happens here because the wikilink substring scan is already walking the body —
no separate pass needed. Only `[[wikilinks]]` count as links; plain
Markdown `[label](other.md)` links do not. The wikilink syntax is the
intentional "this is a cross-doc reference" signal; backlinks reflect that.

**Output:** every source doc has `content` set to rendered HTML and
`links` populated.

---

## 3. Generate

Synthesizes new docs from queries over the rendered corpus: archive pages,
RSS feeds, tag indexes, paginated listings. Generators are templates in
`generators/` whose frontmatter describes a query; each generator fans out
into one or more output docs.

**Data access:** `Arc<Vec<Doc>>` — the full index with markup already
rendered. This is the first phase where reading another doc's `content` is
safe and meaningful.

**Constraint: generators emit pre-rendered output.** A generator's template
produces the final HTML/XML for its output doc directly; the markup phase is
not re-entered. Synthesized docs do not contain raw Markdown bodies that
need a second markup pass.

**Escape hatch: the `markdown` filter.** If a generator wants to author a
section of its output in Markdown (e.g., an RSS item summary written in
prose), it can pipe a string through `{{ body | markdown }}` to run
comrak inline. The filter also expands wikilinks, since it has access
to the `Meta` snapshot. Open question: staleness — if a generator emits a
doc, and a later generator's `markdown` filter expands a wikilink targeting
that newly-emitted doc, what does it see? For now we accept that the
`Meta` snapshot is the source-doc set; generators cannot link to each
other's outputs via wikilink. Revisit if this becomes a real pain point.

**Output:** zero or more new `Doc`s appended to the index. These docs have
their final `content` already set.

---

## 4. Template

Wraps each doc's rendered body in its layout (`layout: foo.html` frontmatter
→ `templates/foo.html`). The full filter set is registered here.

**Data access:** `Arc<Vec<Doc>>` — the full index, including both source
docs and generator outputs, all with rendered content.

**Filters/functions available:** everything markup has, plus `query()` and
`backlinks()`. These functions can do index-wide listings because by this
point the index is stable and meaningful — every doc has its final rendered
content.

**Output:** every doc's `content` becomes its final templated HTML.

---

## 5. Write

Serializes each doc to disk under `build/`, following `doc.output_path`.
`static/` is copied verbatim.

**Data access:** the rendered index; no further mutation.

---

## Cross-cutting: the `Doc → Meta` projection

`Meta` is a strict subset of `Doc`. Implement `From<&Doc> for Meta` (and a
helper for `Vec<&Doc> → Vec<Meta>`) so the markup phase can build its
snapshot with a single pass:

```rust
let metas: Vec<Meta> = index.docs.iter().map(Meta::from).collect();
let snapshot = Arc::new(metas);
```

This costs one projection pass at the start of markup. Cheap — `Meta`'s
fields are all `Clone` and small — and it documents at the type level
exactly what markup is allowed to see.

---

## Why the lifecycle is linear

Earlier iterations exposed `markup::render` so the generate phase could
re-run it on its own emitted docs. That made the phase graph non-linear:
generate could push work back into markup, which made "what does each phase
see" hard to answer. The linear model — generate emits pre-rendered output,
with a `markdown` filter as an escape hatch — keeps each phase's data
contract crisp.
