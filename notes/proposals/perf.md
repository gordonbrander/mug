# Performance optimizations: wikilink index, generate clones, cheap wins

## Context

`italic` is a static-site generator whose build pipeline (`src/build/`) is currently
single-threaded and has a few avoidable hotspots. This plan implements three of
the four findings from `docs/proposals/perf.md` — the ones that are high-value
and low-risk. (Item 2, parallelizing the render phases, is deliberately deferred:
it is the largest change and should wait until a profiler confirms render time
dominates on a real vault.)

The three items here are behavior-preserving optimizations validated against the
borrow checker and the spec's cross-generator visibility semantics:

1. **Wikilink resolution** is O(N²·W): `resolve()` re-slugifies every doc's stem
   on every wikilink lookup. Precompute a stem-slug → candidate index once.
2. **`generate.rs`** clones all matched docs twice (once into `matched`, again
   per page). Drop the redundant first clone.
3. **Cheap wins**: add a release profile to `Cargo.toml`; stop building the
   syntect adapter twice per build.

Out of scope: parallelizing render (item 2 in the proposal) and parallel file
reads (would add a `rayon` dependency for an I/O-bound, marginal win).

---

## Item 1 — Wikilink stem index (O(N²·W) → O(N·W))

**Files:** `src/tera_env.rs`, `src/build/markup.rs`, `src/build/markup/wikilink.rs`

- In `src/tera_env.rs`, add a field to `MarkupEnv` (around line 31):
  `stem_index: std::collections::HashMap<String, Vec<usize>>`.
- Build it once inside `build_markup_env` (tera_env.rs:58) from its existing
  `docs: Arc<Vec<DocMeta>>` param. For each `(i, doc)`, derive the key **exactly**
  as the current candidate filter does (wikilink.rs:162-167):
  `slug::slugify(doc.id_path.file_stem().and_then(|s| s.to_str()).unwrap_or(""))`,
  then `entry(key).or_default().push(i)`. Keying matters: replicate the
  `unwrap_or("")` fallback so behavior is identical (empty-key entries are never
  looked up because an empty `stem_slug` returns `None` early at wikilink.rs:154).
- Change `resolve_in_ast` (wikilink.rs:30) and `resolve` (wikilink.rs:151) to take
  an extra param `stem_index: &HashMap<String, Vec<usize>>`. In `resolve`, replace
  the full `for doc in docs` loop with: `stem_index.get(&stem_slug)` → iterate only
  those indices (`&docs[i]`), applying the existing prefix-match + `dir_distance` +
  lexicographic-`id_path` tiebreak over just that candidate set. The tiebreak is an
  order-independent min, so the winner is identical to today's full scan.
- In `markup::render` (markup.rs:61), pass `&env.stem_index` to `resolve_in_ast`.
  No `&mut env` borrow is live at that point, so the shared borrow is fine
  alongside the existing `&env.options` / `&env.syntect` borrows.

**Note:** `MarkupEnv` does not store the snapshot, and the map holds `usize`
indices (not references), so there is no self-referential-struct problem.

**Test updates (mechanical, ~11 sites in wikilink.rs):** the `render_md` helper
(wikilink.rs:223) must build a stem index from `docs` and pass it; the ~10 direct
`resolve(...)` test calls (wikilink.rs:340–425) each need a locally-built index.
Add a small test helper, e.g. `fn stem_index(docs: &[DocMeta]) -> HashMap<String, Vec<usize>>`,
to keep these terse. `render_doc` in markup.rs tests is unaffected (the map lives
on `env`, built by `build_markup_env`).

---

## Item 3 — Remove redundant clone in `generate.rs`

**File:** `src/build/generate.rs`

- Line 55: change `matched` from `Vec<Doc>` (`.cloned().collect()`) to
  `let matched: Vec<&Doc> = query::evaluate(&g.query, &index.docs);` — drops one
  full clone of every matched doc per generator (a sitemap matching all docs
  currently clones the whole index twice).
- Per page (line 75): clone only the slice:
  `let items: Vec<Doc> = matched[start..end].iter().map(|d| (*d).clone()).collect();`
- **Borrow fix (load-bearing):** `matched` now borrows `&index.docs`, which
  conflicts with `index.insert(doc)` (line 126). Buffer emitted docs into a local
  `Vec<Doc>` inside the page loop, then flush them via `index.insert` **after the
  page loop but still inside the same generator's `for g` iteration**. Because
  `matched` is still in scope at the flush, wrap the matched-using region in a
  block `{ … }` (or `drop(matched)`) so the immutable borrow ends before the
  `&mut index` flush — otherwise it will not compile.
- **Spec invariant preserved:** flushing per-generator keeps cross-generator
  visibility (generate.rs:44-46 — "weight 9999 observes everything emitted by
  earlier generators"). Do NOT defer inserts past the end of the generator loop.
  `markup::render` (line 125) borrows only `markup_env`/`snapshot`, never `index`,
  so it composes with the buffered approach.

No test changes (generate has no direct unit tests; `tests/build.rs` insta
snapshots cover it).

---

## Item 4 — Cheap wins

**4a. Release profile (`Cargo.toml`)** — purely additive, no existing `[profile.*]`:
```toml
[profile.release]
lto = "thin"
codegen-units = 1
```

**4b. Build syntect once (`src/tera_env.rs`, `src/build/markup.rs`)** — currently
built fresh in both `build_markup_env` calls (markup.rs:181, generate.rs:52):
- Add a module-level `static SYNTECT: std::sync::LazyLock<Arc<SyntectAdapter>>`
  in `tera_env.rs`, initialized with the existing
  `SyntectAdapterBuilder::new().theme("InspiredGitHub").build()`.
- Change `MarkupEnv.syntect` from `SyntectAdapter` to `Arc<SyntectAdapter>`;
  `build_markup_env` sets it to `SYNTECT.clone()`.
- Update markup.rs:63 to `Some(env.syntect.as_ref())` (a bare `&env.syntect` on an
  `Arc` field won't coerce to `&dyn SyntaxHighlighterAdapter`).
- `SyntectAdapter` is `Send + Sync` (enforced by comrak's trait), so the static is
  sound; it also persists across `watch`/`serve` rebuilds, which is a bonus.
- No test changes — no test reads `env.syntect`.

---

## Verification

1. `cargo test` — unit tests (esp. `wikilink` and `tera_env`) plus the
   `tests/build.rs` insta integration snapshots must pass unchanged. The snapshot
   suite is the key behavioral guard for items 1 and 3 (rendered HTML, wikilink
   resolution, generator/pagination output must be byte-identical).
2. `cargo build --release` — confirm the new profile compiles clean.
3. Run a real build against a fixture vault (`cargo run -- build` in a scaffolded
   site, or one of the `tests/` fixtures) and diff the `output/` against a
   pre-change build to confirm zero output differences.
4. (Optional) Time `cargo run --release -- build` on the largest available vault
   before/after to quantify the wikilink and clone wins.
