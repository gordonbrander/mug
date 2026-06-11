# Proposal: ATProto PDS publishing

Add optional support for publishing italic sites to an ATProto Personal Data
Server (PDS) via the [standard.site](https://standard.site/) lexicons.

## Short answer

Yes, it's possible, and **a static site generator does not need to become a
dynamic server to publish to a PDS.** The key is separating two roles that are
easy to conflate.

## The role confusion to untangle

A PDS *is* a dynamic server — it stores a user's repository, manages identity,
signs records, and exposes XRPC APIs. But **you don't write or host the PDS.**
The user already has one (their Bluesky account's PDS, or a self-hosted one).
What italic would implement is a **client** that *writes records into* an
existing PDS over HTTP.

Writing to a PDS is just authenticated API calls:

- `com.atproto.server.createSession` → get an access token
- `com.atproto.repo.putRecord` / `applyWrites` → write `site.standard.publication`
  and `site.standard.document` records
- `com.atproto.repo.uploadBlob` → upload images, returning blob refs you embed in
  records

A CLI tool that runs locally can make those calls perfectly well. This is the
same shape as `italic deploy` pushing to S3/Netlify — a publish step that talks
to a remote API. **No long-running server, no dynamic rendering on italic's
part.** It fits the SSG model cleanly: `italic build` produces output, and an
`italic publish` (or similar) step syncs records to the PDS.

## Where the "dynamic vs static" question actually bites

The thing that genuinely needs a dynamic endpoint is **identity/DID
resolution**, and even that is static-hostable:

- **`did:web`** resolution is literally serving a static `/.well-known/did.json`
  file — fully compatible with static hosting.
- **`did:plc`** is resolved via the PLC directory (plc.directory), not your site,
  so it needs nothing from you.
- **Publication verification** in standard.site uses a static
  `/.well-known/site.standard.publication` file pointing to your AT-URI, plus
  HTML `<link>` tags in documents — all of which italic can emit as static files
  during build.

So the only "server-ish" obligations are static files italic already knows how
to generate.

## How visitors see the content

standard.site deliberately defines **metadata only** — the standard leaves
actual content format to individual platforms. That gives italic two
non-exclusive models:

1. **Keep serving your static HTML as you do today** (GitHub Pages, Netlify,
   etc.), and *additionally* write standard.site records to the PDS purely for
   discoverability/portability in the ATmosphere. Readers in the wild still hit
   your static site; aggregators/AppViews and other ATProto apps (Leaflet, Pckt,
   Offprint) can discover, index, recommend, and port your content via the
   records.
2. **Let an AppView render from the records.** Apps that understand the lexicon
   can render your documents from PDS data directly. That rendering is *their*
   dynamic server, not italic's.

Either way, italic stays a static generator. The PDS is the canonical store for
portable records; rendering for humans is handled by your static host and/or
third-party AppViews.

## Relevant lexicons

- `site.standard.publication` — metadata for a publication (URL, name, icon,
  description, theme)
- `site.standard.document` — defines what a document contains
- `site.standard.graph.subscription` — user connections to publications
- `site.standard.graph.recommend` — recommendations for documents

Each lexicon is independent: implement only what's needed. For a v1, the
publication + document records are the meaningful pair.

## What this would look like in italic

- A new optional publish target (alongside the existing build): map each page's
  front matter → a `site.standard.document` record; the site config → a
  `site.standard.publication` record.
- Upload image assets as blobs, rewrite refs.
- Emit the `.well-known` verification files into the static output.
- Store the user's handle/DID + app-password (or OAuth token) in config/env; do
  incremental sync (only `putRecord` changed docs, keyed by rkey/CID).

A plausible module layout (per the project's `foo.rs` + sibling `foo/`
convention):

```
src/publish.rs          // publish target dispatch
src/publish/atproto.rs  // XRPC client + session auth
src/publish/atproto/record.rs  // page/site -> lexicon record mapping
```

## Open design decisions

- **Auth:** app-password vs. the newer OAuth flow.
- **Record keys (rkeys):** how to derive stable rkeys from pages so
  re-publishing *updates* rather than duplicates. (Slug-derived rkey keyed by a
  stored map, or a content-addressed scheme.)
- **Incremental sync:** track last-published CID per page to avoid rewriting
  unchanged records.
- **Blob lifecycle:** garbage-collecting orphaned blobs when images are removed.

## Bottom line

PDS publishing is a client concern, not a hosting concern. italic remains a
static generator; it just gains an API-driven publish step plus a few static
`.well-known` files. The only dynamic infrastructure involved (the PDS itself,
AppViews) is owned by the user or the network, not by italic.

## References

- [standard.site](https://standard.site/)
- [Personal Data Server (PDS) — AT Protocol Community Wiki](https://atproto.wiki/en/wiki/reference/core-architecture/pds)
- [Self-hosting — AT Protocol Docs](https://atproto.com/guides/self-hosting)
- [Creating a did:web atproto account using goat — bryan newbold](https://whtwnd.com/bnewbold.net/3mdc7fpbxhk26)
