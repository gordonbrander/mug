# The build pipeline

`italic build` runs a linear pipeline of stages, each consuming the document
index the previous stage left behind. Knowing the order explains most
"why does my template see X but not Y" questions.

## The stages

1. **Read** — scan `content/` into the document index, parsing frontmatter.
   Documents with `draft: true` are dropped *here* unless drafts are included
   (`serve`/`watch`, or `build --drafts`) — so drafts stay out of every later
   stage: collections, taxonomies, backlinks, archives, all of it. It's as if
   the file weren't there.
2. **Classify collections** — evaluate each collection's query against
   frontmatter. This happens *before* markup so that step 3 can fill members.
3. **Apply defaults** — fill each collection member's missing frontmatter from
   the collection's `defaults:` entry.
4. **Render markup** *(parallel)* — each body runs through a content-phase
   Tera render (macros, partials), then the Markdown renderer, with wikilink
   resolution and the optional hashtag pass (which adds to each doc's `tags`).
   Resolved wikilinks are recorded as the doc's outgoing links.
5. **Classify taxonomies and backlinks** — taxonomy terms are bucketed *after*
   markup (the hashtag pass can add terms), and the wikilink graph is inverted
   into backlinks. The index is now complete and is **frozen** — nothing
   mutates it from here on.
6. **Archives** *(parallel)* — templates in `archives/` run over the frozen
   index, emitting view pages (paginated listings, feeds, sitemaps). Archives
   read only the classification of source content, never each other's output,
   so they are order-independent and run in parallel.
7. **Template** *(parallel)* — every source document and archive page is
   wrapped in its layout, producing the final output.
8. **Write** — outputs land in `output_dir` (default `public/`).
9. **Static copy** — `static/` (theme first, then site, so site files win) is
   copied over the top.

The parallel stages (markup, archives, template) are embarrassingly parallel —
this is where italic's speed comes from.

## Consequences worth knowing

**Two Tera phases with different powers.** Stage 4 renders document bodies
through Tera *before* the index is complete, so content-phase templates can't
list other pages — no `collection()`, `all()`, `taxonomy()`, `doc()`,
`backlinks`, or `related` inside a document body. Layouts (stage 7) run against
the frozen index and get everything. The
[template reference](../reference/templates.md#the-two-phases) marks each
filter's availability.

**Collections see raw frontmatter; taxonomies see hashtags.** A collection
query can only match what's in frontmatter (it runs pre-markup). Taxonomy
classification runs post-markup, so `#hashtags` lifted from body text do count
as `tags` terms.

**Defaults exist before bodies render.** Because defaults apply at stage 3, a
content-phase template inside a document body already sees them (e.g.
`page.data` keys defaulted by its collection).

**Drafts vanish entirely.** Dropped at stage 1, a draft can't appear in a
collection count, a tag page, a feed, or anyone's backlinks.

**Archives can't see each other.** An archive page can't link to or list
another archive's output. They read the same frozen snapshot of source
content.

## See also

- [Content model](content-model.md)
- [Archives guide](../guides/archives.md)
- [CLI reference](../reference/cli.md)
