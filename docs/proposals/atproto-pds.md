# Proposal: ATProto PDS publishing

Add optional support for publishing italic sites to an ATProto Personal Data
Server (PDS) via the [standard.site](https://standard.site/) lexicons.

## Short answer

Yes, it's possible, and **a static site generator does not need to become a
dynamic server to publish to a PDS.** The key is separating two roles that are
easy to conflate.

The work splits cleanly into two halves of very different character:

1. **Static artifacts** (the verification glue) — fits italic's existing build
   pipeline almost perfectly. Low effort, deterministic, offline.
2. **Record publishing** (pushing records to a PDS) — a genuinely *new kind* of
   operation for italic: networked, stateful, and authenticated. This is where
   the real work and the design decisions are.

The architectural consequence: italic today is a **pure, offline, embarrassingly
parallel, deterministic** transform (`content/` → `public/`). Publishing to the
atmosphere is none of those things. So this should be a **new `italic publish`
command** that runs *alongside* `build` — reusing the build pipeline to get a
fully-classified `DocIndex`, then layering a networked sync step on top — rather
than a stage inside the build.

## The role confusion to untangle

A PDS *is* a dynamic server — it stores a user's repository, manages identity,
signs records, and exposes XRPC APIs. But **you don't write or host the PDS.**
The user already has one (their Bluesky account's PDS, or a self-hosted one).
What italic would implement is a **client** that *writes records into* an
existing PDS over HTTP.

Writing to a PDS is just authenticated API calls (all under `com.atproto.*`,
POST, HTTP/JSON, auth via the `Authorization` header):

- `com.atproto.server.createSession` → exchange handle + app password for an
  access token (the legacy path; see [Auth](#auth) for the OAuth alternative).
- `com.atproto.repo.putRecord` → create-or-update a record at a known `rkey`.
  Ideal for an SSG that keys records by a stable slug so re-publishing updates
  in place. Supports `swapRecord`/`swapCommit` for optimistic concurrency.
- `com.atproto.repo.createRecord` → create with a server-assigned `rkey`
  (returns `{uri, cid}`).
- `com.atproto.repo.applyWrites` → a **batch** of create/update/delete ops in a
  single atomic commit. Best for publishing a whole site at once.
- `com.atproto.repo.deleteRecord` → remove a record (for unpublished pages).
- `com.atproto.repo.uploadBlob` → POST raw bytes with a `Content-Type`; returns
  a blob ref `{$type:"blob", ref:{$link:cid}, mimeType, size}` that you then
  embed in a record (e.g. `coverImage`/`icon`). Blobs are uploaded *first*, then
  referenced in the record write.

A CLI tool that runs locally can make those calls perfectly well. This is the
same shape as `italic deploy` pushing to S3/Netlify — a publish step that talks
to a remote API. **No long-running server, no dynamic rendering on italic's
part.** It fits the SSG model cleanly: `italic build` produces output, and an
`italic publish` step syncs records to the PDS.

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

## The AtProto addressing model

A few terms recur throughout the implementation:

- **DID** — a stable cryptographic account ID (e.g. `did:plc:abc123`). **Handle**
  — a human-readable alias (e.g. `alice.com`) that resolves to a DID.
- A **repo** (one per account, hosted on the user's PDS) is a key/value store
  partitioned into **collections** keyed by **NSID** (the lexicon type, e.g.
  `site.standard.document`).
- Within a collection, each record has an **rkey** (record key — often a TID
  timestamp, but can be any stable string like `self` or a slug).
- An **AT-URI** — `at://<did>/<collection-nsid>/<rkey>` — addresses a single
  record. A **CID** is the content hash, used for integrity and
  optimistic-concurrency swaps.
- A **lexicon** is the schema language describing records and XRPC methods.
  Records carry a `$type` discriminator equal to their NSID. The PDS validates
  optimistically by default (validates against the lexicon if known, else allows
  the write).

## Relevant lexicons

Under the `site.standard` namespace:

- `site.standard.publication` — the site/blog itself (record).
- `site.standard.document` — an individual post/page (record).
- `site.standard.graph.subscription` — a user following a publication.
- `site.standard.graph.recommend` — endorsing/recommending a document.
- Helper defs: `site.standard.theme.basic`, `site.standard.theme.color`.

Each lexicon is independent: implement only what's needed. For a v1, the
**publication + document** records are the meaningful pair.

### `site.standard.document`

- **Required:** `site` (string — an `at://…publication` URI *or* an `https://`
  URL for loose docs), `title` (string), `publishedAt` (datetime).
- **Optional:** `path` (combined with the site/url to build a canonical URL),
  `description`, `coverImage` (blob, <1 MB), `content` (open union, `$type`
  required), `textContent` (plaintext), `bskyPostRef`, `tags` (array), `links`,
  `labels`, `contributors` (array of `{did, role?, displayName?}`), `updatedAt`.
- Note: there is **no `createdAt`** — use `publishedAt`/`updatedAt`.

```json
{
  "$type": "site.standard.document",
  "site": "at://did:plc:abc123/site.standard.publication/3lwafzkjqm25s",
  "path": "/blog/getting-started",
  "title": "Getting Started with Standard.site",
  "description": "Learn how to use Standard.site lexicons",
  "coverImage": { "$type": "blob", "ref": {"$link": "bafkrei..."}, "mimeType": "image/jpeg", "size": 245678 },
  "textContent": "Full text of the article...",
  "tags": ["tutorial", "atproto"],
  "publishedAt": "2024-01-20T14:30:00.000Z"
}
```

### `site.standard.publication`

- **Required:** `url` (base URL of the publication), `name`.
- **Optional:** `description`, `icon` (blob, ≥256×256), `basicTheme` (ref to
  `site.standard.theme.basic` with rgb colors), `labels`, `preferences` (object,
  e.g. `showInDiscover: bool`).

## Verification artifacts (the static half)

standard.site requires two **domain-ownership proofs**, both of which are static
files italic already knows how to emit:

1. **Publication proof:** serve `https://yourdomain.com/.well-known/site.standard.publication`
   returning the publication's AT-URI as text:
   `at://did:plc:…/site.standard.publication/<rkey>`.
2. **Per-document proof:** inject a link tag into each published page's HTML
   `<head>`:
   `<link rel="site.standard.document" href="at://did:plc:…/site.standard.document/<rkey>" />`

Both fit the existing pipeline directly:

- The `.well-known` file is structurally identical to the existing
  `sitemap.xml` / `rss.xml` generators — a generated `Doc` with `DocKind::Raw`
  that passes through Tera but skips comrak.
- The `<link>` injection is a markup/template concern (a hook in the template
  phase, or a small markup pass).

Both are deterministic and offline, so unlike the record sync they *can* live in
the build, gated behind a `publish:`/standard.site config block so they only
appear when configured.

## Mapping italic's `Doc` to `site.standard.document`

No new content modeling is required — `Doc` already carries nearly everything
the lexicon wants. It's a serialization of existing fields:

| `site.standard.document` field | Source in `Doc` |
|---|---|
| `title` (required) | `doc.title` |
| `publishedAt` (required, RFC3339) | `doc.date` |
| `site` (required, AT-URI) | the publication record's URI (from publish state) |
| `updatedAt` | `doc.updated` |
| `path` | derived from `doc.output_path` + `config.base_path` |
| `description` | `doc.summary` |
| `tags` | keys of `doc.terms["tags"]` |
| `textContent` / `content` | `doc.content` (plaintext vs. markdown/HTML union) |
| `coverImage` (blob) | a frontmatter field → `uploadBlob` first, then embed ref |

The publication record maps from the `site:` config sub-map (`name` ← title,
`url` ← `site.url`, `description`, `icon` blob from a configured path).

## Rust crates

The `atrium-rs` ecosystem is the maintained mainstream. Tokio is **already** a
dependency (the `serve` command), so async networking is not a new burden.

- **`atrium-api`** — actively maintained; generated request/response types for
  all `com.atproto.repo.*` methods (`create_record`, `put_record`,
  `apply_writes`, `upload_blob`). Provides `AtpAgent` (session management +
  `agent.login()`) and a lower-level `AtpServiceClient`.
- **`atrium-xrpc`** / **`atrium-xrpc-client`** — core XRPC traits and HTTP client
  impls (e.g. a `reqwest`-backed client).
- **`atrium-oauth`** — OAuth session/credential management.
- **`esquema`** (`esquema-cli`, `esquema-codegen`) — a fork of atrium-codegen
  that generates Rust types from **custom** lexicon JSON. **This is the key
  piece for standard.site:** run it over the `site.standard.*` lexicon JSON to
  get typed `site::standard::document::RecordData` structs usable directly in
  atrium's repo methods (via CLI or `build.rs`), instead of hand-rolling JSON.
  Consumed from GitHub (not yet on crates.io).
- Reference: **Rusty Statusphere**
  (`fatfingers23/rusty_statusphere_example_app`) — an end-to-end Rust app using
  `atrium-oauth` + `atrium-api` + `esquema`-generated types to create
  custom-lexicon records. The canonical "how to push records from Rust" example.

The `createRecord` call pattern in atrium is roughly:
`{ collection: Document::NSID.parse()?, repo: did.into(), rkey, record: doc.into() }`.

> **Gap to flag:** there does not appear to be a single canonical repo hosting
> the `site.standard.*` lexicon JSON (the docs site documents them per-page but
> doesn't link a definitions repo). For an implementation you'd transcribe the
> lexicon JSON from the docs, or borrow it from a consuming project (the Astro or
> Jekyll plugins), to feed into esquema.

## The new state concept

This is the crux of what makes `publish` different from `build`: build holds **no
memory between runs**, but publish must.

To make re-publishing *update* records instead of creating duplicates, italic
must remember the `rkey` (and ideally the CID, for optimistic `swapRecord`)
assigned to each doc, plus the publication record's AT-URI. Two options:

- A **sidecar state file** (e.g. `.italic/atproto.json`: `id_path → {rkey, cid}`,
  plus the publication URI), or
- Write the AT-URI back into each doc's **frontmatter**.

A stable, **content-derived `rkey`** (e.g. slug-based) is cleaner than the random
TIDs AtProto assigns by default, because the mapping stays reconstructible even
if the state file is lost.

## What this would look like in italic

- A new `italic publish` command (in `src/command/`), wired through `lib.rs` /
  `main.rs` alongside `build`, `serve`, etc. It runs the build pipeline to obtain
  a frozen `DocIndex`, then hands it to the publish layer.
- A publish layer that maps each page's `Doc` → a `site.standard.document` record
  and the `site:` config → a `site.standard.publication` record.
- Upload image assets as blobs first, then rewrite the refs into the records.
- Bootstrap the publication record on first run (`putRecord`), capture its
  AT-URI into state.
- Emit the `.well-known` verification file and inject the `<link>` tags during
  build (gated on config).
- Read the user's handle/DID + app password (or OAuth token) from env / a
  gitignored credentials file — **never** `config.yaml`.
- Incremental sync: `putRecord` only changed docs, keyed by stored rkey/CID.

A plausible module layout (per the project's `foo.rs` + sibling `foo/`
convention):

```
src/command/publish.rs           // CLI verb: build, then dispatch to publish layer
src/publish.rs                   // publish orchestration + state file
src/publish/atproto.rs           // XRPC client + session/OAuth auth
src/publish/atproto/record.rs    // Doc/site -> lexicon record mapping
src/publish/state.rs             // id_path -> {rkey, cid} sidecar
src/build/markup/standard_link.rs   // <link rel="site.standard.document"> injection
```

(The `.well-known` generator can live alongside the existing sitemap/RSS
generators rather than under `publish/`, since it's a pure build artifact.)

## Auth

The hardest decision, and partly out of italic's hands:

- AtProto's **officially recommended** path is **OAuth**, and it's the long-term
  direction. But the OAuth profile is built around an interactive browser
  redirect plus DPoP (proof-of-possession request signing) — awkward for a
  headless CLI/SSG. There's no standardized client-credentials/service flow. The
  practical pattern is to run the flow once interactively and persist/refresh the
  token.
- **App password + `com.atproto.server.createSession`** is the pragmatic
  near-term path: exchange handle + app password for a session JWT, then call
  the repo write endpoints with the access token. Simplest to implement, still
  functional, but officially **deprecated** long-term.

Either way italic needs a secrets story (PDS host + handle + app password or
OAuth tokens via env vars / a gitignored credentials file).

**Recommendation:** ship v1 with app-password auth to get working quickly;
treat OAuth as a fast-follow.

## Open design decisions

- **Auth:** app password now vs. investing in the OAuth + DPoP flow up front.
- **Record keys (rkeys):** stable slug-derived rkeys (reconstructible) vs.
  server-assigned TIDs (requires the state map to be authoritative).
- **State location:** sidecar `.italic/atproto.json` vs. frontmatter write-back.
- **Write strategy:** `putRecord` per doc (simple, supports incremental) vs.
  `applyWrites` batch (one atomic commit for the whole site).
- **Sync scope for v1:** incremental ("publish only changed docs," needs change
  detection against stored CIDs) vs. "republish everything every run" (simpler,
  more PDS writes).
- **Blob lifecycle:** garbage-collecting orphaned blobs when images are removed.
- **Config surface:** a `publish:` / standard.site block — which collection(s)
  to publish, the handle/PDS host, and publication metadata (name, URL, icon).

## Rough effort

- Static artifacts (`.well-known` generator + `<link>` injection) and the
  `Doc` → record serialization: **~1 day**, low risk, idiomatic italic.
- atrium/esquema client + publication bootstrap + a working `italic publish` with
  app-password auth and a sidecar state file: **~2–4 days** for a solid v1.
- OAuth + DPoP, blob/image handling, batch atomic writes, and incremental change
  detection: each adds incremental time on top.

So a usable prototype is **a few days**; a polished, OAuth-based, incremental
implementation is more like **1–2 weeks**.

## Bottom line

PDS publishing is a client concern, not a hosting concern. italic remains a
static generator; it just gains an API-driven publish step plus a few static
`.well-known` files. The only dynamic infrastructure involved (the PDS itself,
AppViews) is owned by the user or the network, not by italic. The static half
slots into the existing build pipeline; the networked, stateful half lives in a
new `italic publish` command beside it.

## References

### standard.site

- [standard.site](https://standard.site/)
- [Introduction](https://standard.site/docs/introduction/)
- [Quick start (verification flow)](https://standard.site/docs/quick-start/)
- [`site.standard.document` lexicon](https://standard.site/docs/lexicons/document)
- [`site.standard.publication` lexicon](https://standard.site/docs/lexicons/publication)
- [jekyll-standard-site (reference plugin)](https://github.com/andrew/jekyll-standard-site)
- [Standard.site: the publishing gateway](https://stevedylan.dev/posts/standard-site-the-publishing-gateway/)

### AtProto

- [Lexicon spec](https://atproto.com/specs/lexicon)
- [OAuth spec](https://atproto.com/specs/oauth) · [OAuth for AtProto (blog)](https://docs.bsky.app/blog/oauth-atproto)
- [XRPC spec](https://atproto.com/specs/xrpc) · [endpoint reference](https://endpoints.bsky.app/)
- [Personal Data Server (PDS) — AT Protocol Community Wiki](https://atproto.wiki/en/wiki/reference/core-architecture/pds)
- [Self-hosting — AT Protocol Docs](https://atproto.com/guides/self-hosting)
- [Creating a did:web atproto account using goat — bryan newbold](https://whtwnd.com/bnewbold.net/3mdc7fpbxhk26)

### Rust crates

- [atrium-rs](https://github.com/atrium-rs/atrium) · [atrium-api docs](https://docs.rs/atrium-api)
- [esquema (custom-lexicon codegen)](https://github.com/fatfingers23/esquema)
- [Rusty Statusphere example app](https://github.com/fatfingers23/rusty_statusphere_example_app)
</content>
</invoke>
