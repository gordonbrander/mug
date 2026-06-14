# Italic vs Hugo: Feature Gap Analysis

*2026-06-13*

## Framing

Italic and Hugo aren't the same kind of tool. **Italic is a digital-garden SSG** —
its differentiated value is wikilinks, backlinks, related-notes scoring, hashtags,
and zero-config Obsidian compatibility. In those areas italic *beats* Hugo (Hugo
needs plugins/render-hooks to approximate them). Hugo is a **general-purpose SSG**
with a decade of accreted features.

So a gap is only interesting if it's something italic's *actual users* (people
publishing notes/blogs/wikis) would hit. The gaps below are sorted into three tiers
by that test, with a note on which are probably **out of scope by design** vs.
**worth closing**.

---

## Where italic already wins (for context)

These are Hugo gaps, not italic gaps — keep them as the moat:

- **Wikilinks** with Obsidian-style fuzzy stem matching + directory-distance
  disambiguation. Hugo has no native wikilinks.
- **Backlinks** graph. Hugo: none built-in.
- **Related pages** scored over taxonomies *and* the link graph, weighted. Hugo's
  `.Site.RegularPages.Related` is keyword-only and weaker.
- **Hashtag extraction** from body → tags taxonomy. Hugo: none.
- **YAML-as-document** input type. Hugo: data-only.
- **Two-phase Tera** (restricted markup phase → full template phase) — a clean model
  Hugo lacks.

---

## Tier 1 — Real gaps, cheap to close, high value

Table-stakes SSG features that italic's users *will* miss, none requiring
architectural change.

### 1. Built-in RSS/Atom feeds
Italic has **no native feed** — you template `archives/feed.xml` by hand. Hugo
auto-generates RSS for every section, taxonomy, and the home page.
**Recommendation:** ship a default feed template in the scaffold (you already
scaffold sitemap examples), or add a built-in `feed` output. For a blog-aware garden
tool, "RSS just works" is expected.

### 2. Built-in sitemap.xml + robots.txt
Same story — templatable via archives but not automatic. Hugo generates
`sitemap.xml` out of the box. Cheap to make a zero-config default.

### 3. Aliases / redirects
**The most important missing feature for a digital garden.** Gardens constantly
rename and reorganize notes, which breaks URLs. Hugo's `aliases: [/old-path/]`
frontmatter emits redirect stub pages automatically. Italic has nothing. Given
wikilinks already let you move files freely *internally*, the lack of external
redirect generation is a sharp edge. **Strong recommend.**

### 4. Scheduled / dated publishing
Italic has only `draft: true`. Hugo has `publishDate` (future-dated → excluded until
date) and `expiryDate` (auto-unpublish). For a blog-aware tool this is a common need.
The `date` field already exists; gating on it is small.

### 5. Table of contents
No built-in TOC. Hugo exposes `.TableOfContents` from heading parsing. comrak can emit
heading anchors; surfacing a `page.toc` (or heading list) would be a small,
frequently-requested addition for long notes.

### 6. Heading anchors / auto-IDs
Related: confirm headings get stable `id` slugs for deep-linking (Hugo does by
default). Essential for wikilink-to-heading and shareable note sections.

---

## Tier 2 — Moderate effort, on/near the roadmap

Genuinely useful, larger lift; some already have proposals.

### 7. Asset pipeline (CSS/SCSS, JS, minify, fingerprint)
Italic copies `static/` verbatim — no SCSS, no minification, no content-hash
fingerprinting for cache-busting. Hugo Pipes does all of this (`css.Sass`,
`js.Build` via esbuild, `resources.Fingerprint`, `.Minify`). The existing
`proposals/assets.md` (swc-based, JS-only, Phase 1) explicitly excludes CSS/SCSS and
fingerprinting — which are arguably *more* commonly needed than JS bundling for a
notes site. Consider whether SCSS + fingerprint should lead instead of JS bundling.

### 8. Image processing
No resize/crop/convert/EXIF. Hugo's `images.*` + `.Resize`/`.Fill` produce responsive
images and WebP. For garden sites heavy on screenshots/diagrams this matters, but it's
a big dependency (image crates). Probably Tier 2 "later."

### 9. Generalized pagination
Pagination today is **archive-only** (`per_page`). Hugo lets any list template call
`.Paginate` over an arbitrary collection. If someone wants a paginated index page
that isn't an archive, italic can't. Consider exposing pagination as a template
helper, not just an archive feature.

### 10. Markdown render hooks (esp. images & links)
Hugo's render hooks let you wrap every image in a `<figure>`, lazy-load, or route
every link through a transform. Italic already intercepts links for wikilinks but
offers no general image/link/heading hook. Useful for responsive images,
external-link icons, etc.

### 11. Math + diagrams (KaTeX/Mermaid)
No built-in math passthrough or Mermaid/GoAT diagrams. Hugo supports both via
passthrough + render hooks. For a thinking/notes tool, LaTeX math is a common ask.
Could be done today via macros + client-side JS, but it's not documented as a path —
worth a guide at minimum.

### 12. Page bundles / co-located resources — ✅ shipped
Hugo's leaf bundles let a note keep its images in the same folder and reference them
relatively, with the bundle moving as a unit. Italic separated `content/` from
`static/`, which was a real friction point for Obsidian users (who keep attachments
beside notes).

**Resolved** via co-located media (spec §8.1): any non-content file under
`content/` is copied to the matching output path, and `![](image.png)`,
`![[image.png]]`, and `[[report.pdf]]` references resolve to it — permalink-safe.
This is the "image next to the .md" model rather than Hugo's leaf bundle (the page
stays a single doc; assets mirror alongside). See
[the authoring guide](../guides/authoring.md#co-located-media-images-and-attachments).
Note transclusion (`![[Some Note]]` inlining another note's body) remains out of
scope.

---

## Tier 3 — Large scope; likely *deliberately* out of scope

Document these as explicit non-goals rather than gaps, so the positioning is clear.

| Hugo feature | Italic | Verdict |
|---|---|---|
| **i18n / multilingual** (per-language trees, translation linking, `i18n` strings, `lang.FormatNumber`) | None | Probably out of scope for v1; but state it. Single biggest "Hugo can, italic can't." |
| **Output formats system** (one page → HTML+JSON+AMP+custom media types) | Archives can emit `.xml/.json/.txt`, but no per-page multi-output | Niche for gardens. Skip. |
| **Theme modules / composition** (Hugo Modules, multi-theme stacking, mounts) | Single theme, one overlay level | Fine as-is; the overlay model is simpler and adequate. |
| **Build environments** (`--environment`, per-env config, prod vs dev config) | None | Minor. A `--env`/config-merge could be cheap later. |
| **`hugo deploy`** (S3/GCS/Azure) | None | Out of scope — rsync/CI is fine. Hugo's is rarely used anyway. |
| **Menu system** (nested menus from frontmatter/config) | Manual via `data/` YAML | Acceptable; maybe sugar later. |
| **`resources.GetRemote`** (fetch remote data/assets at build) | None | Out of scope. |
| **Pluggable markup** (AsciiDoc/reST/org) | Markdown + HTML only | Explicit non-goal in spec §2. Good. |

---

## Smaller, concrete gaps worth a line each

- **Syntax highlighting is not configurable** — fixed `InspiredGitHub` theme, always
  on, no line numbers or line-range highlighting. Hugo's Chroma exposes theme, line
  numbers, `hl_lines`, etc. The TODO already lists "make syntax highlighting optional."
- **No `cascade`-down-the-tree** — italic's per-collection `defaults` are close to
  Hugo's `cascade`, but Hugo cascades down section trees by path with target filters.
  Worth confirming defaults cover nested-section inheritance (there's an `05_cascade`
  fixture — verify it matches Hugo's mental model or rename to avoid the implication).
- **No built-in 404 handling convention** — Hugo renders `404.html`. Probably already
  works via a normal page, but document it.
- **Cross-reference ergonomics** — `doc(id_path)` exists; Hugo's `ref`/`relref` differ.
  Minor.
- **No EXIF/asset metadata, no SRI/Subresource Integrity** — ties to asset pipeline
  absence.

---

## Recommended priority order

To close the gaps italic's own audience hits most:

1. **Aliases/redirects** (Tier 1 #3) — most garden-specific, currently a sharp edge.
2. **Built-in RSS + sitemap defaults** (Tier 1 #1–2) — expected baseline, cheap.
3. ~~**Co-located page resources / Obsidian attachments** (Tier 2 #12)~~ — ✅
   shipped; strengthened the headline "publish your Obsidian vault" claim.
4. **Scheduled publishing + TOC + heading anchors** (Tier 1 #4–6) — small,
   high-frequency wins.
5. **SCSS + fingerprinting** ahead of JS bundling in the assets proposal (Tier 2 #7).
6. **Document the deliberate non-goals** (i18n, output formats, modules) so the
   comparison reads as *focus*, not *deficiency*.
