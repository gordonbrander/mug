# TypeScript / JS bundling: an `assets` build stage

## Context

`italic` currently has no story for JavaScript or TypeScript beyond `static/`,
which is copied verbatim by [`static_copy`](../../src/build/static_copy.rs).
That means no transpilation (browsers can't run `.ts`), no bundling (every
`import` is a separate request), and no minification. This is the first open
item in `TODO.md`: *"Typescript compile / JS bundling."*

This proposal adds an **`assets` stage** to the build pipeline
(`src/build.rs`) that compiles and bundles JS/TS. It is split into two phases
so the high-value, low-risk work ships first and the heavy, identity-defining
work (a networked package resolver) is a deliberate, separate decision.

The guiding constraint is Italic's stated identity (`README.md`): *"no framework
churn, no dependencies, does one thing well."* Phase 1 honors that — one
embedded bundler, no network, no package manager. Phase 2 is where Italic would
*optionally* take on more, and it is explicitly deferred until we decide the
tradeoff is worth it.

### Why a bundler doesn't solve dependency *fetching*

A bundler (swc, oxc, esbuild) only combines modules **already on disk**. Its
module resolution walks the filesystem looking for `node_modules/`; it never
fetches anything. So "where do `npm:` / `jsr:` libraries come from?" is a
*separate* problem from bundling — package management — and Phase 1 sidesteps it
entirely by adopting Hugo's model: **bring your own `node_modules`**. Phase 2 is
the answer for the cases where that isn't enough.

---

## Phase 1 — swc bundling, no custom resolvers (ship this)

Embed swc, add an `assets` stage that transpiles + bundles configured
entrypoints into `output_dir`. Resolution is swc's stock node-style resolver:
relative imports and (if present on disk) `node_modules`. Italic fetches nothing.

### Dependencies (`Cargo.toml`)

Add the swc umbrella crate with the bundler/transform/codegen features:

```toml
swc_core = { version = "...", features = [
    "ecma_bundler",
    "ecma_transforms",      # TS strip, JSX, target downleveling
    "ecma_codegen",
    "ecma_parser",
    "ecma_minifier",
    "common",
] }
swc_ecma_loader = { version = "...", features = ["node"] }  # NodeModulesResolver
```

These are heavy crates (long first compile, large tree). That cost is accepted
**only** because it buys a self-contained, single-binary build with no Node
toolchain required at runtime. Pin exact versions; swc's internal crates
version-lock together. (We use the `swc_core` umbrella precisely to get a
coherent, co-versioned set rather than hand-aligning a dozen `swc_ecma_*`
crates.)

> Note: choosing swc here is partly forward-looking. Phase 2 uses Deno's
> `deno_ast`, which *itself* wraps swc — so committing to swc now keeps the two
> phases in one parser/AST family rather than carrying both swc and oxc.

### Source layout & config

Introduce a dedicated **`assets_dir`** (default `assets/`), distinct from
`static_dir`. This keeps the contract crisp and avoids `static_copy` shipping
raw `.ts` files:

- `static/` → copied verbatim (unchanged).
- `assets/` → compiled; only the *configured entrypoints* produce output.

New optional `config.yaml` block (all keys optional, sensible defaults — matches
the zero-config ethos):

```yaml
assets_dir: assets            # top-level key, mirrors content_dir/static_dir

assets:
  entrypoints:                # paths relative to assets_dir; [] = stage is a no-op
    - js/main.ts
  minify: true                # default: false (or true only in `build`, see below)
  sourcemap: false            # emit .js.map alongside output
  target: es2020              # downlevel target passed to swc transforms
```

**Files:**

- `src/config.rs`: add `assets_dir: PathBuf` (default `PathBuf::from("assets")`)
  to `Config` + `Default`. Add an `AssetsConfig` struct (`entrypoints:
  Vec<PathBuf>`, `minify: bool`, `sourcemap: bool`, `target: String`) stored as
  `pub assets: AssetsConfig` with `#[serde(default)]`. Parse it from the raw
  mapping the same way `defaults`/`site` are pulled in `Config::load`
  (config.rs:65-82), or — since `assets` is a plain typed struct with no special
  uplift — just let `#[serde(default)]` on the field deserialize it directly
  (simpler; no manual extraction needed). Add unit tests mirroring the existing
  `defaults_block_is_parsed` / `defaults_absent_yields_empty` pattern.

### New stage: `src/build/assets.rs`

Mirror the shape of `static_copy::run(config)` — it takes only `&Config`, since
asset compilation is independent of the doc `Index`.

```rust
pub fn run(config: &Config) -> Result<()> {
    if config.assets.entrypoints.is_empty() {
        return Ok(());               // zero-config / no-JS sites: pure no-op
    }
    // For each entrypoint (parallelize with rayon, like the render phases):
    //   1. Resolve abs path under config.assets_dir.
    //   2. Bundle with swc:
    //        - Globals + Lrc<SourceMap>
    //        - Resolver: CachingResolver<NodeModulesResolver> (relative + node_modules)
    //        - Loader: read file from disk, parse via swc TS/JS syntax by extension
    //        - swc_bundler::Bundler::new(...).bundle(entries)
    //   3. Apply TS strip / target transforms + optional minify (swc_ecma_minifier).
    //   4. Emit via swc_ecma_codegen::Emitter -> String (+ sourcemap if enabled).
    //   5. Compute output path: mirror the entrypoint's path under assets_dir
    //      into output_dir, swapping the extension to `.js`
    //      (e.g. assets/js/main.ts -> public/js/main.js). create_dir_all the
    //      parent, then fs::write — same write idiom as static_copy.rs:25-31.
}
```

Key decisions baked in:

- **"No custom resolvers"** means we ship swc's `NodeModulesResolver` *only*. It
  handles `./relative` and bare specifiers against an on-disk `node_modules/`. We
  add **no** `npm:` / `jsr:` / `https:` scheme handling — those throw a clear
  "unsupported import scheme (see Phase 2)" error, so behavior is honest, not
  silently broken.
- **BYO `node_modules`.** If an entrypoint imports `lodash`, the user must have
  run `npm install` (or `bun`/`pnpm`) themselves. A missing dependency is a hard
  build error with the unresolved specifier named. Italic never installs anything.
- **Parallelism.** Bundle entrypoints concurrently with `rayon`
  (`entrypoints.par_iter()`), consistent with the Rayon work in `#8`. swc's
  `Globals`/`SourceMap` are per-bundle, so each entrypoint is an independent unit
  — no shared mutable state across the parallel iterator.
- **Failure mode.** A parse/resolve/bundle error returns `Err` with context
  (which entrypoint, which specifier), bubbling through `build::run`. In `watch`
  it is logged and the watcher keeps running (watch.rs:72-79 already does this).

### Pipeline wiring (`src/build.rs`)

Add the module and call it last, after `static_copy`:

```rust
pub mod assets;
// ...
static_copy::run(&config)?;
assets::run(&config)?;     // compiled JS lands in output_dir
```

Update the stage list in the module doc comment (build.rs:1-9) to add stage 7,
`assets — compile assets/ entrypoints into output_dir`. Ordering after
`static_copy` means compiled output deterministically wins over any colliding
verbatim file (it shouldn't collide, since `assets/` ≠ `static/`, but the
ordering is well-defined regardless).

### Watch integration (`src/command/watch.rs`)

Add `assets_dir` to the watched `dirs` array (watch.rs:39-45) so editing a
`.ts` source triggers a rebuild. v1 is full-rebuild (per spec §2), so no
incremental asset logic is needed — `rebuild()` re-runs the whole pipeline,
which now includes `assets::run`. `node_modules/` is intentionally **not**
watched (large, churny, and changes only on explicit installs).

### Scaffold (`src/command/scaffold.rs`, `scaffold/`)

Optionally add a tiny `assets/js/main.ts` and the matching `assets:` block to
the scaffolded `config.yaml`, so `italic new` demonstrates the feature. Keep it
minimal (a `console.log` and a relative `import`) to show transpile + bundle
without introducing an npm dependency in the starter site.

### Phase 1 verification

1. `cargo test` — new `config.rs` unit tests for `assets`/`assets_dir` parsing
   and defaults; existing suite unchanged.
2. Add `tests/` coverage (insta or a focused integration test): a fixture with
   `assets/js/main.ts` importing `./util.ts`, assert `public/js/main.js` exists,
   contains both modules inlined, and has no `.ts` syntax / no remaining
   `import './util'`.
3. Zero-config guard: a site with **no** `assets:` block and **no** `assets/`
   dir must build byte-identically to today (stage is a no-op). The existing
   `tests/build.rs` snapshots are the guard.
4. BYO `node_modules`: a fixture with a vendored `node_modules/leftpad`, assert
   the bare import resolves and inlines. A second fixture importing a missing
   package must fail with the specifier named.
5. `cargo build --release` — confirm the swc dependency compiles clean under the
   existing LTO profile; note the first-build time delta in the PR.

### Phase 1 scope boundaries (explicit non-goals)

- No `npm:` / `jsr:` / `https:` scheme imports (Phase 2).
- No package fetching / install / lockfile. Italic is not a package manager.
- No type-checking. swc *strips* types; it does not run `tsc`. Document this —
  users who want type errors run `tsc --noEmit` themselves.
- No CSS/SCSS/PostCSS pipeline (separate proposal if ever wanted).
- No content-hash fingerprinting / SRI (could be a small Phase 1.5).

---

## Phase 2 (deferred) — `deno_ast` + `deno_graph` for `npm:` / `jsr:` / `https:`

Deferred, and gated on an explicit decision to accept the tradeoff below. This
phase makes Italic resolve Deno-style specifiers — `npm:lodash@4`,
`jsr:@std/encoding`, `https://esm.sh/...` — without the user maintaining a
`node_modules/`.

### Approach

Replace (or augment) Phase 1's swc-direct path with Deno's open-source,
MIT-licensed resolution crates, which are compatible with Italic's AGPL-3.0 license:

- **`deno_graph`** — builds the module graph from entrypoints; natively
  understands `npm:`, `jsr:`, `https:`, `node:`, `data:`. We implement its
  `Loader` trait (fetch URL / read file) and optionally `Resolver` (import maps).
- **`deno_ast`** — parse + transpile (wraps swc, so Phase 1's swc investment
  carries over rather than being thrown away).
- **`deno_npm` + `deno_semver`** — npm semver resolution → dependency snapshot.
- **`node_resolver`** — `package.json` `exports`/`imports`/conditions.
- **`deno_cache_dir`** — on-disk HTTP cache (the `DENO_DIR` model).
- **`deno_lockfile`** — reproducible builds (pin + integrity hashes).
- **`import_map`** — `deno.json`-style `imports` for pinning/redirects.

These crates hand us exactly the correctness-critical algorithms (semver
resolution, `exports` resolution, graph building, the fetch cache) that are
miserable to implement correctly and are battle-tested against the whole
npm/jsr ecosystem. We supply only the IO glue.

### What this *costs* (why it's deferred, not default)

This is a **values decision**, not a technical blocker:

1. **Dependency weight & churn.** The Deno crate set is large and its API is not
   a stability promise (`deno_graph` is pre-1.0, version-locks internally, breaks
   on Deno's cadence). Carrying it is in direct tension with the README's "no
   framework churn, no dependencies" — it relocates the churn rather than
   removing it.
2. **Network at build time.** `italic build` would now reach the network on cold
   cache. This makes the cache + lockfile *mandatory* (reproducibility,
   offline/CI builds), not optional.
3. **Inheriting Deno's opinions.** CJS↔ESM interop, condition resolution, how
   `npm:` maps to a cache layout — behavior becomes "whatever Deno decided" and
   shifts under us. Mostly that's good (correct, tested); it's also no longer
   ours to control.

### Cheaper alternative within Phase 2 (note for the decision)

If the full Deno stack feels too heavy, a lighter middle path supports the same
*specifiers* via CDN delegation: one cached HTTP `Loader` + rewrite rules
(`npm:foo` → `https://esm.sh/foo`, `jsr:@s/n` → esm.sh/jsr or JSR's HTTPS API) +
a small lockfile. esm.sh resolves semver ranges for us. This keeps the
dependency tree tiny at the cost of trusting a CDN and delegating resolution
semantics to it. Worth weighing against `deno_graph` when Phase 2 is picked up.

### Phase 2 is *not* required for Phase 1

Phase 1 stands alone and ships value (transpile + bundle local TS, plus BYO
`node_modules`). Phase 2 only changes *where dependencies come from*; it does not
change the `assets` stage's place in the pipeline, its config surface (it
*extends* it), or its output contract. Nothing in Phase 1 needs to anticipate
Phase 2 beyond the swc/`deno_ast` family alignment already noted.

---

## Open questions

- **`assets_dir` vs `static/`:** confirm a dedicated `assets/` dir is preferred
  over a convention like "compile `static/**/*.ts` in place." Dedicated dir is
  cleaner (no verbatim-copy collision) and is the assumption above.
- **Minify default:** off everywhere, or on for `build` and off for
  `watch`/`serve`? (Faster, readable dev output; minified releases.)
- **Fingerprinting:** is content-hashed output (`main.[hash].js`) wanted in
  Phase 1, and if so how do templates reference the hashed name? (Likely its own
  small follow-up — needs a manifest the Tera env can read.)
- **Phase 1.5 candidates:** sourcemaps polish, SRI/fingerprint, a `js` Tera
  function so templates can reference compiled bundles by source path.
