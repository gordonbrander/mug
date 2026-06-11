# Troubleshooting

Common surprises and how to read them.

## A wikilink renders as plain text (`span.nolink`)

The target didn't resolve, so it rendered as `<span class="nolink">` instead
of a link. To find unresolved links, grep the output:
`grep -r 'class="nolink"' public/`. Usual causes:

- **Typo or stem mismatch** — the link matches against the slugified filename
  stem, so `[[My Note]]` needs a file whose stem slugifies to `my-note`.
- **The target is a draft** — drafts are invisible to the link graph in
  production builds, so links to them un-resolve until they ship. Under
  `italic serve` (drafts included) the same link works. See
  [Drafts](guides/drafts.md).
- **Wrong file matched or path prefix mismatch** — with duplicate stems the
  closest directory wins; disambiguate with the prefixed form
  `[[dir/Name]]` (anchored at the content root). See
  [Wikilinks](guides/wikilinks.md#how-targets-resolve).

Also remember wikilinks inside code spans/fences are intentionally left
literal.

## A template error fails the build

Tera reports the template name and what went wrong; the messages are usually
literal. Things worth knowing:

- A missing variable is an error — guard optional values with `{% if %}`
  (e.g. `pagination.prev_url`, a `doc(...)` lookup, custom `page.data` keys).
- `doc(id_path=...)` returns `null` for unknown paths rather than erroring;
  most other functions fail loudly — `collection(name="typo")` and `all()`
  with any argument are build errors by design.
- Config typos also fail loudly: an unknown collection query key, an unknown
  `related:` key, or a `defaults:` entry naming an undeclared collection all
  stop the build with a pointer.

## My page rendered without its layout

A document with no `template:` (and no collection default supplying one)
renders its body as the final output. Set `template:` in frontmatter or in
`defaults:` for the collection. Check the collection actually matches the
file: `path:` globs match against the path relative to `content/`.

## `collection()`/`backlinks`/`related` "unknown function/filter" inside a document body

Document bodies render in the content phase, before the page index exists, so
index-reading functions are template-phase only. Move that logic into the
layout. See
[the build pipeline](concepts/build-pipeline.md#consequences-worth-knowing).

## Styles or links break when deployed under a subpath

Hosting at `example.com/blog/` needs `base_path: /blog` under `site:`, and
templates must build URLs with the [URL filters](reference/templates.md#url-filters)
(`relative_url`, `link`, …) rather than hardcoded `/`-prefixed paths.

## Two pages landed on the same output path

Permalinks don't collide-check across documents; the last write wins. Watch
for two files whose patterns expand identically (e.g. same `:slug` and date in
one collection). Make patterns more specific, or include `:yyyy/:mm/:dd`.

## Dates are wrong

Without frontmatter, `date` falls back to file *created* time, then modified
time — and file timestamps don't survive every `git clone` or CI checkout
(files get checkout-time stamps). For anything date-sensitive (blog ordering,
dated permalinks), set `date:` in frontmatter or via collection defaults.

## Drafts showed up where I didn't expect (or vanished where I did)

`serve` and `watch` always include drafts; `build` never does unless you pass
`--drafts`. There is no per-command override beyond that. See
[Drafts](guides/drafts.md).

## Stale output after changing permalinks or deleting pages

`italic build` writes into `output_dir` without clearing it, so renamed or
deleted pages can leave orphans. Run `italic clean && italic build` for a
fresh tree (and use `rsync --delete` or equivalent when deploying).

## Still stuck?

Open an issue at <https://github.com/gordonbrander/italic/issues> with the
command you ran and the full error output.
