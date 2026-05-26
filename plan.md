# Plan: knead — Static Site Generator

- Spec: spec.md

## Overview

**Goals**: Build a zero-config, Rust-based static site generator whose every site-specific behavior lives in `config.yaml`, frontmatter, and Tera templates. The pipeline reads `content/`, renders bodies, generates virtual pages from `generators/`, and templates everything against a single in-memory index that supports path/tag/backlink queries. Targets the surface defined in spec.md §3–§13.

**Approach**: Land thin, demoable vertical slices, but **front-load the type skeleton** so every phase is shaping a known structure rather than inventing one. Phase 1 lands the core types (`Doc`, `Config`, `Index`, the four phase function signatures) *and* a minimal end-to-end build through all four phases over a single Markdown file. Subsequent phases either populate fields the `Doc`/`Index` already carry or fill in a phase whose signature is already in place. The binary stays runnable and verifiable after every phase. Functional core (pure `Doc` operations, query evaluation, permalink expansion) sits behind an imperative shell (filesystem walking, Tera registry, output writing). Integration fixtures under `tests/fixtures/` drive verification — every phase ships at least one fixture site whose expected output is asserted.

**Tech**: Rust 2024 edition, single binary crate. Standard library plus the dependencies enumerated below.

### Dependencies

Pulled in up front so the type skeleton in Phase 1 can be modeled against real crate types (e.g. `chrono::DateTime`, `serde_yaml_ng::Mapping`) rather than placeholders that get swapped later. The list is intentionally small — the spec's "small, predictable surface area" goal applies to our dependency graph too.

- **clap** (with `derive` feature) — CLI parsing. Derive-macro subcommands (`build`, `watch`, `new`) keep `main.rs` declarative. Mature, low-churn; the obvious default.
- **walkdir** — recursive directory traversal for `content/`, `generators/`, `data/`, `static/`. Simple, no-config, handles symlinks correctly. Plain `std::fs::read_dir` recursion would also work but `walkdir` is a one-liner.
- **pulldown-cmark** — CommonMark parser. Rust-native, pull-based, fast, the de-facto choice. Used in the markup phase to render `.md` bodies into `doc.content`. Enable the `html` feature.
- **serde** (with `derive`) + **serde_yaml_ng** — frontmatter, `config.yaml`, and `data/` cascade. We pick **`serde_yaml_ng`** rather than the original `serde_yaml` because the upstream crate is unmaintained (deprecated by its author); `serde_yaml_ng` is the community fork with the same API. `Doc.data` is typed as `serde_yaml_ng::Mapping` so all frontmatter survives uplift verbatim.
- **chrono** (with `serde` feature) — `DateTime<Utc>` for `doc.date` / `doc.updated`. Lands in Phase 1 because those fields exist on `Doc` from the start, even though Phase 2 is the first time they carry real values. Used again in Phase 6 for `:yyyy/:mm/:dd` permalink expansion.
- **anyhow** — application-level error plumbing. We are a binary, not a library, so structured error enums (via `thiserror`) are not worth the ceremony; `anyhow::Result<T>` with `.context(...)` annotations is enough.
- **tera** — Jinja-like runtime template engine. A *runtime* engine is mandatory here (users supply their own templates, so compile-time engines like `askama` are unsuitable). Tera natively supports macros (the spec's shortcode mechanism, Phase 11), custom filters (`query`, `backlinks`, `permalink`), and template inheritance. We construct two `Tera` instances — one with the restricted filter set for the markup phase, one with the full set for the template phase — so the spec §11 restricted-markup invariant is enforced by construction.
- **slug** — sluggify document stems and wikilink targets. Used for the `:slug` permalink variable and again in Phase 9 for wikilink resolution. Tiny crate, no transitive deps worth noting.
- **globset** — compile glob patterns once and match many paths cheaply. Backs the `path:` operator in queries. Picked over `glob` because `globset` is built for repeated matches against a large set of paths, which is exactly the query-evaluation pattern.
- **notify** (v6+, with the default debouncer) — cross-platform file watcher. Coalesces filesystem events on macOS (FSEvents), Linux (inotify), and Windows so we get one rebuild per editor save, not three.

**Dev-dependencies:**

- **insta** — snapshot testing for fixture-driven integration tests. Each phase ships a fixture site under `tests/fixtures/<phase>/` whose built output is compared against a stored snapshot, with `cargo insta review` for accepted updates. Avoids hand-maintaining `expected/` trees as the renderer evolves. Could alternatively roll our own string-diff helper, but `insta`'s review workflow pays for itself by Phase 3.

**Explicitly not pulling in:**

- *gray_matter* / dedicated frontmatter crates — `---`-delimited splitting is ~20 lines of code; not worth the dependency.
- *thiserror* — we have no public library API; `anyhow` is sufficient.
- *regex* — wikilink scanning is a simple state machine; a hand-rolled scanner avoids a heavy transitive dep.
- *tokio* / async runtime — the build is CPU-bound and inherently sequential across phases; `rayon` may be worth considering inside the markup phase later but is not in v1.

## TODO

- [x] Phase 1: Type skeleton + four-phase pipeline — land the core data types (`Doc`, `Config`, `Index`) and stubbed signatures for all four phases (`read`, `markup`, `generate`, `template`), wired into a `build` command that renders one Markdown file end-to-end through every phase. Most `Doc` fields exist but stay at defaults; most phases are near-passthrough. The shape is right, the contents grow later.
  - [x] Add `clap` (`derive`), `walkdir`, `pulldown-cmark` (`html`), `serde` (`derive`), `serde_yaml_ng`, `chrono` (`serde`), `anyhow` to `[dependencies]`; add `insta` to `[dev-dependencies]`. (Note: `insta` pulled in but not yet exercised — the integration harness uses a hand-rolled tree diff for now.)
  - [x] Add `src/config.rs::Config` with `content_dir`, `output_dir`, `templates_dir`, `static_dir`, `data_dir`, `generators_dir`; `Default` impl returns spec §3 names; no file load yet.
  - [x] Add `src/doc.rs::Doc` with **all fields from spec §5** present from day one: `id_path: PathBuf`, `output_path: PathBuf`, `template: Option<String>`, `title: String`, `content: String`, `tags: Vec<String>`, `date: DateTime<Utc>`, `updated: DateTime<Utc>`, `data: serde_yaml_ng::Mapping`. Add a `Doc::from_body(id_path, body) -> Doc` constructor that fills defaults so Phase 1 can use it without frontmatter.
  - [x] Add `src/index.rs::Index` with just `docs: Vec<Doc>`. Simplest thing that could work: no secondary indexes yet. Query functions in later phases iterate over `docs` directly; we'll add indexes if and when iteration is actually a bottleneck. Provide `Index::insert(doc)`.
  - [x] ~~Add `src/phases/mod.rs` re-exporting~~ — **deviation**: per CLAUDE.md (modern Rust prefers `foo.rs` + sibling `foo/` over `foo/mod.rs`), the phase modules live at the crate root (`src/read.rs`, `src/markup.rs`, `src/generate.rs`, `src/template.rs`, `src/write.rs`) rather than nested under a `phases/` parent. Subsequent phases should target these top-level paths.
  - [x] Add `src/read.rs::run(&Config) -> Index` — walks `content_dir`, dispatches `.md` only, builds `Doc` via `Doc::from_body`, inserts into `Index`. (Other extensions handled in later phases.)
  - [x] Add `src/markup.rs::run(&mut Index)` — for each doc, render `.md` body via `pulldown-cmark` into `doc.content`. (Tera, frontmatter handling, and per-type branching land in later phases.)
  - [x] Add `src/generate.rs::run(&Config, &mut Index)` — empty stub (no-op) with a doc comment marking Phase 8.
  - [x] Add `src/template.rs::run(&Config, &mut Index)` — passthrough; `doc.content` is the final output. (Tera lands in Phase 3.)
  - [x] Add `src/write.rs::run(&Config, &Index)` — writes each doc's `content` to `output_dir.join(doc.output_path)`. Default `output_path` in Phase 1 mirrors `id_path` with `.html` extension.
  - [x] Replace `src/main.rs` hello-world with a `clap` dispatcher; `build` calls `read → markup → generate → template → write` in order. (The orchestration lives in `src/lib.rs::build()` so the integration harness can call it directly; `main.rs` is a thin CLI shim.)
  - [x] Add `tests/fixtures/01_skeleton/` with one `content/hello.md` and the expected `dist/hello.html`.
  - [x] Add `tests/build.rs` integration harness (helper `run_build(fixture_name)`) that runs the pipeline against a fixture and diffs the output tree against `expected/`. (Implementation `chdir`s into a temp copy of the fixture under a `Mutex` guard since `build()` reads `Config::default()` relative to cwd.)
  - [x] Verify: `cargo build` succeeds with no `dead_code` warnings on the new types (use `#[allow(dead_code)]` where genuinely deferred).
  - [x] Verify: `cargo test` passes the `01_skeleton` fixture.
  - [x] Verify: manual `cargo run -- build` in `tests/fixtures/01_skeleton/` produces `dist/hello.html`.

- [x] Phase 2: Frontmatter uplift — populate the `Doc` fields that Phase 1 left at defaults by parsing the `---`-delimited YAML block.
  - [x] Add `src/frontmatter.rs::split` returning `(Option<&str>, &str)` for the YAML block and body, plus `parse(&str) -> serde_yaml_ng::Mapping`.
  - [x] ~~Add `Doc::from_source(id_path, source, fs_meta)`~~ — **deviation**: layered constructor API instead — `Doc::new(id_path, content, data)` (pure uplift from data), `Doc::parse(id_path, source)` (splits frontmatter, calls new), `Doc::load(content_dir, id_path)` (reads file, parses, applies fs-based date fallback). Also added `impl Default for Doc`. fs fallback in `load` is keyed off whether `doc.data` contains a parseable `date`/`updated` (not a sentinel check on the field) so `1970-01-01` written in frontmatter still wins.
  - [x] Update `src/read.rs` to use `Doc::load` instead of `Doc::from_body` (collapses the prior `fs::read_to_string` + constructor into one call).
  - [x] Add unit tests for `frontmatter::split` (empty, missing closing `---`, body-only, CRLF, empty block, no-body-after-fence, fence-with-trailing-text) and for `Doc::new`/`parse`/`load` (each uplift rule, malformed YAML, fs fallback).
  - [x] Add `tests/fixtures/02_frontmatter/` with a doc declaring `title`, `tags`, `date`; expected output is the rendered body only (still no template wrapping).
  - [x] Verify: `cargo test` passes (23 unit tests + 2 integration fixtures).
  - [x] Verify: manual build of `02_frontmatter` strips the frontmatter from output.

- [x] Phase 3: Tera template phase — fill in the `src/template.rs` stub with real Tera rendering; load `templates/`; `doc.template` selects a template. Markup phase also runs Tera over bodies (restricted: no `query`/`backlinks` registered yet).
  - [x] Add `tera` to `Cargo.toml`.
  - [x] Add `src/tera_env.rs` with `build_markup_env(&Config) -> Tera` (restricted) and `build_template_env(&Config) -> Tera` (full). Both load `templates/**/*.{html,xml}` via a single brace-expansion glob (Tera's globwalk-backed `Tera::new` handles it). Missing `templates/` returns `Tera::default()` so fixtures without a templates dir still work. Filter registration is the only difference; the registry is the seam later phases extend.
  - [x] Update `src/markup.rs`: for `.md` bodies run restricted Tera over the body string before Markdown render. Signature gained `&Config`. Also added `#[derive(Serialize)]` on `Doc` so it can be inserted into a `tera::Context` directly.
  - [x] Update `src/template.rs`: for each `Doc`, render its `doc.template` (default: passthrough — `None` skips this phase entirely) against a context `{ doc, page: { content: doc.content } }`. `page` uses a small local `Page<'a>` struct to avoid a `serde_json` direct dep.
  - [x] Add `tests/fixtures/03_templates/` with `templates/base.html`, a doc declaring `template: base.html`, and expected wrapped output.
  - [x] Generalized `tests/build.rs::run_build` once: now copies every entry in the fixture dir except `expected/`. Unblocks `templates/` for Phase 3 and `static/`/`data/`/`config.yaml`/`generators/` for later phases without touching the harness again.
  - [x] Verify: `cargo test` passes (29 unit tests + 3 fixtures); negative unit tests in `tera_env::tests` confirm the markup env errors when a body calls `query()` or `backlinks()`.
  - [x] Verify: manual `cargo run -- build` in `tests/fixtures/03_templates/` produces a byte-identical `dist/post.html`.

- [x] Phase 4: HTML and YAML input types — extend the `read` phase dispatch and the markup branch table. The `Doc` shape is unchanged; this slice just teaches the existing pipeline to produce a `Doc` from `.html` and `.yaml` sources.
  - [x] Added `DocKind { Markdown, Html, Yaml }` plus `Doc::kind(&self) -> DocKind` that derives the kind from `id_path.extension()` — no field on `Doc` to keep in sync; single source of truth is the extension. Default arm returns `Markdown` but is unreachable in practice (`read::run` filters extensions first).
  - [x] Widened `src/read.rs` extension filter to `matches!(ext, "md" | "html" | "yaml")`. Per-extension construction lives in `Doc::load`, which dispatches to `Doc::parse` for md/html and a new `Doc::parse_yaml` for yaml (whole file is the data map; `content` field, defaulting to `""`, becomes the body).
  - [x] `src/markup.rs` branches on `doc.kind()`: Markdown runs Tera + `pulldown_cmark`, Html/Yaml run Tera only (no Markdown pass). The Tera context build is shared across all arms — the only branch is the post-Tera Markdown step.
  - [x] Added `tests/fixtures/04_input_types/` with one `.md`, one `.html`, one `.yaml` source; each body contains `{{ doc.title }}` to prove restricted Tera ran for all three kinds. Expected outputs prove Markdown wraps in block-level HTML, HTML passes through, YAML's `content` field is the body source.
  - [x] Verify: `cargo test` passes (36 unit tests + 4 integration fixtures, including `input_types`).
  - [x] Verify: manual `cargo run -- build` in `tests/fixtures/04_input_types/` produces three `.html` files matching `expected/` byte-for-byte.

- [x] Phase 5: Static dir + `config.yaml` + `data/` cascade — extend `Config` with loaded `site` data, introduce a `SiteData` carrier for `site` + `data/`, and pass it to the template context.
  - [x] Add `src/config.rs::Config::load(path) -> Result<(Config, Mapping)>` — parses `config.yaml` into the typed `Config` (with `#[serde(default)]` so missing dir keys fall back to defaults) and returns the `site:` sub-map alongside. Absent file yields `(Config::default(), Mapping::new())`.
  - [x] Add `src/site_data.rs::SiteData { site: Mapping, data: Mapping }`. **Deviation from original plan**: `data/` is loaded as **top-level files only** — `data/menu.yaml` → `data.menu`; subdirectories are skipped silently. Field renamed `config` → `site` to match the nested-`site:` decision and the Tera variable name.
  - [x] Thread `SiteData` through the pipeline (signature change on `markup::run` and `template::run`); inject `site` and `data` into both Tera contexts (markup and template).
  - [x] Add `src/static_copy.rs::run(&Config)` — walkdir-copy from `static/` to `output/`; called after `write` in `lib::build()`. Missing `static/` is a no-op.
  - [x] Add `tests/fixtures/05_cascade/` with `static/robots.txt`, `config.yaml` declaring `site.title` + `site.description`, `data/menu.yaml`, and `templates/base.html` referencing all three.
  - [x] Verify: `cargo test` passes (45 unit tests + 5 fixtures).
  - [x] Verify: manual `cargo run -- build` in `tests/fixtures/05_cascade/` copies `static/robots.txt` and renders config + data values into `dist/index.html` byte-identical to `expected/`.

- [x] Phase 6: Permalinks — fill in the `Doc::output_path` slot with real `permalink` expansion. `Doc` already carries `output_path` from Phase 1; this slice replaces the default-mirror computation with `permalink:` expansion of `:slug`, `:yyyy`, `:mm`, `:dd`.
  - [x] Add `slug` to `Cargo.toml`.
  - [x] Add `src/permalink.rs::expand` supporting the variable set from spec §5.1; trailing `/` means write `index.html`. **Deviation**: signature is `expand(pattern: &str, id_path: &Path, date: &DateTime<Utc>) -> PathBuf` rather than `expand(pattern, &Doc)` — the call site in `Doc::new` doesn't yet have a `Doc`, and primitives are easier to unit-test in isolation.
  - [x] Default behavior (no `permalink`) still mirrors `id_path` (now factored as `permalink::default_for(&id_path)`).
  - [x] ~~Move `output_path` resolution from `write::run` into the end of `read::run`~~ — **deviation**: the resolution actually lived in `Doc::new`, not `write::run`. Kept it there (via a private `resolve_output_path` helper) so every construction path — `parse`, `parse_yaml`, future generator-emitted docs — populates `output_path` for free. `Doc::load` re-runs the helper after fs date fallback so date-templated permalinks still resolve correctly when `date:` is omitted from frontmatter.
  - [x] Add `tests/fixtures/06_permalinks/` with a post declaring `permalink: /blog/:yyyy/:slug/` and the expected `dist/blog/2025/hello/index.html`.
  - [x] Add unit tests for `permalink::expand` (date components, trailing slash, default fallback, leading-slash strip, verbatim no-trailing-slash).
  - [x] Verify: `cargo test` passes (52 unit tests + 6 integration fixtures).

- [x] Phase 7: `query` filter — add a `Query` type and evaluator that iterates over `Index.docs` linearly; register the filter on the template env only. No secondary indexes — simplest thing that could work; revisit if iteration cost actually shows up.
  - [x] Add `globset` to `Cargo.toml`.
  - [x] Add `src/query.rs::Query { path: Option<Glob>, tag: Option<String>, order_by: OrderKey, sort: SortDir, limit: Option<usize> }`. **Deviations**: (1) added a `limit` field (extends spec §9 with a small predictable result cap); (2) the Tera surface is **kwargs only** — `Query::from_kwargs(&HashMap<String, tera::Value>) -> tera::Result<Self>`. No compact-string parser yet; that grammar is deferred to Phase 8, where serde can deserialize the same fields from a YAML `query:` map in generator frontmatter for free.
  - [x] Add `src/query.rs::evaluate(query: &Query, docs: &[Doc]) -> Vec<&Doc>` — linear scan, filter, sort, then `truncate(limit)`. **Deviation**: signature takes `&[Doc]` instead of `&Index` because the function operates on a frozen `Arc<Vec<Doc>>` snapshot, not the live index (see template-phase wiring below).
  - [x] Register `query` as a Tera function on the template env only; the markup env stays restricted.
  - [x] Snapshot wiring: `template::run` clones `index.docs` into an `Arc<Vec<Doc>>` at the start of the phase and passes it to `build_template_env(config, snapshot)`. `Doc` gained `#[derive(Clone)]` to make this possible. The snapshot is what every template's `query()` call sees.
  - [x] Add `tests/fixtures/07_query/` with three posts (`posts/{a,b,c}.md` at three distinct dates) and a `home.html` template using `{% for p in query(path="posts/*.md") %}` with no `order_by`/`sort`, so the C→B→A order in the expected output proves the default is `created desc`.
  - [x] Verify: `cargo test` passes (64 unit tests + 7 integration fixtures).
  - [x] Verify: existing `tera_env::tests::markup_env_does_not_register_query` (and `…backlinks`) still pass — these guard the spec §11 "no index-backed filters in markup" invariant. Plus a new `template_env_registers_query` positive test.
  - [x] Verify: manual `cargo run -- build` in `tests/fixtures/07_query/` produces `dist/index.html` listing the posts in date-desc order.

- [x] Phase 8: Generators + pagination — fill in the `src/generate.rs` stub left in Phase 1. Introduce `Generator` and `Pagination` types; emit `Doc` descriptors that join the existing `Index`.
  - [x] Add `src/generator.rs::Generator { id_path, query, per_page: Option<usize>, permalink: String, template: Option<String>, weight: i32, body: String, data: Mapping }`. **Deviation**: renamed spec.md's `order` field to `weight` to disambiguate from `query.order_by`. Also kept the full frontmatter `Mapping` so generator templates can reference arbitrary author-supplied fields via `{{ doc.data.xxx }}`.
  - [x] Add `src/generator.rs::Pagination { current: usize, total: usize, prev_url: Option<String>, next_url: Option<String>, items: Vec<Doc> }`. **Deviation**: `items` is `Vec<Doc>` (full clones) rather than `Vec<DocId>` because there is no `permalink` filter yet to resolve IDs back to URLs/titles in templates. Phase 9 may revisit this.
  - [x] Add `Query::from_yaml_mapping(&Mapping) -> Result<Self>` (in `src/query.rs`) so generator frontmatter's `query:` sub-mapping uses the same field names as the Tera `query(...)` kwargs.
  - [x] Implement `src/generate.rs::run` — walk `generators/`, parse each via `Generator::parse`, sort by `weight` ascending, evaluate the query for each, chunk by `per_page`, build a `Doc` per page with `pagination` serialized into `doc.data`. **Deviation**: `generate::run` signature gained `&SiteData` so emitted docs can be markup-rendered with the same context shape as authored content.
  - [x] After fan-out, call `markup::render` on each emitted descriptor and insert into the index. Refactored `markup::run`'s per-doc body into a public `markup::render(env, site_data, doc)` helper for this reuse. Both markup and template phases now also inject `doc.data.pagination` as a top-level `pagination` context key.
  - [x] Extend `permalink::expand` to support `:page` (added `page: Option<usize>` parameter). Also added `permalink::to_url(output_path) -> String` for prev/next URL building (Phase 9 will reuse it for the `permalink` filter).
  - [x] **Deviation**: `Doc::kind()` default arm changed from `Markdown` to `Html` so generator-emitted `.xml` (sitemap, feed.xml, etc.) bypasses pulldown-cmark. Authored content is unaffected because `read::run` filters to `md|html|yaml`.
  - [x] Generator-emitted docs use their resolved `output_path` as `id_path` so sitemap-style queries naturally include them.
  - [x] Add `tests/fixtures/08_generators/` with five posts, a `blog.html` generator (per_page=2 → 3 pages), and a `sitemap.xml` generator (`weight: 9999`).
  - [x] Verify: `cargo test` passes (76 unit + 8 integration fixtures). Three blog pages emitted with the expected `prev:/blog/page-N/` and `next:/blog/page-N/` URLs and item slices.
  - [x] Verify: `dist/sitemap.xml` lists the three `blog/page-N/index.html` paths after the five posts — proof that the high-`weight` sitemap sees docs emitted by the lower-`weight` blog generator.

- [x] Phase 9: Wikilinks + `permalink` filter — resolve `[[Wiki Link]]` / `[[Wiki Link|Display]]` in markup; register `permalink` filter on both envs. Backlinks are computed on demand by linear scan in Phase 10 — no graph stored on `Index`.
  - [x] Added `src/wikilink.rs::expand(body, &Doc, &[Doc]) -> (String, Vec<PathBuf>)` — scan-and-replace pass run between Tera and pulldown-cmark (Markdown only). **Deviation**: signature takes `&[Doc]` (slice of the markup-phase snapshot) and returns the outlinks vec, rather than `&mut Index`. The caller (`markup::render`) assigns `doc.outlinks` from the return value. Decouples the scanner from `Index` mutation and lets it operate on the frozen `Arc<Vec<Doc>>` snapshot — same shape Phase 7 introduced for `query`.
  - [x] Added `outlinks: Vec<PathBuf>` field on `Doc` (populated by wikilink expansion, consumed by Phase 10 backlinks).
  - [x] Resolution implemented in `wikilink::resolve`: slug the target, walk source `id_path`'s parent chain (current dir, then upward, terminating at root), first stem-slug match wins.
  - [x] **Deviation from plan re unresolved behavior**: resolved targets render as `<a class="wikilink" href="…">display</a>` and unresolved targets render as `<span class="nolink">display</span>` (plus a stderr warning, build succeeds) — both classes are author-stylable. The plan's "plain text fallback" was tightened to a semantic `<span>` per user clarification.
  - [x] Registered `permalink` Tera filter on both envs via new `tera_env::register_permalink(&mut env, Arc<Vec<Doc>>)`. **Decision**: `permalink` ships on both markup and template envs (spec §11 only restricts index-*listing* filters like `query`/`backlinks`; a 1:1 id_path → URL lookup is safe at body-render time). Filter takes a string `id_path` and returns the corresponding doc's URL via `permalink::to_url`. Errors loudly if the id_path is unknown.
  - [x] **Wiring deviation**: `build_markup_env` signature widened to `(&Config, Arc<Vec<Doc>>) -> Result<Tera>`. `markup::run` and `generate::run` each take their own `Arc::new(index.docs.clone())` snapshot at phase start and thread it through `markup::render`. The markup snapshot's stale `content`/`outlinks` fields are unobserved (resolver and filter read only `id_path` and `output_path`).
  - [x] Added `tests/fixtures/09_wikilinks/` with `content/hello.md`, `content/perma.md` (exercises `permalink` filter), `content/blog/2025/deep.md` (links `[[Hello]]` up, `[[Sibling]]` same-dir, `[[Sibling|click me]]` display form, `[[Missing]]` unresolved), and `content/blog/2025/sibling.md`.
  - [x] Verify: `cargo test` passes (95 unit + 9 integration fixtures). 16 new wikilink unit tests cover scanner edge cases (unmatched open, newline-in-brackets, nested open, HTML escape, outlink dedup, closer-dir wins, slug normalization) and 3 new tera_env tests cover the permalink filter on both envs plus the unknown-id_path error path.
  - [x] Verify: manual `cargo run -- build` in `tests/fixtures/09_wikilinks/` emits exactly one `warning: unresolved wikilink [[Missing]] in blog/2025/deep.md` line on stderr; `dist/` matches `expected/` byte-for-byte.

- [ ] Phase 10: `backlinks` filter — register `backlinks` on the template env only; accepts `order_by` (`title` | `created` | `updated`) and `sort`.
  - [ ] Add `backlinks` filter implementation in `src/tera_env.rs` — for a given doc id, linear-scan `index.docs` and collect any whose `outlinks` contain that id. Sort and return. No persistent graph.
  - [ ] Add `tests/fixtures/10_backlinks/` where doc `a.md` links to `b.md` via wikilink and `b.md` renders its backlinks as a list.
  - [ ] Verify: `cargo test` confirms `b.html` lists `a` in its backlinks block.
  - [ ] Verify: negative test asserts `backlinks` is unavailable in markup.

- [ ] Phase 11: Tera macros for markup — author-defined macros in `templates/macros/**/*.html` are auto-imported into the markup-phase env so Markdown bodies can call them as shortcodes; macros run before Markdown render per spec §6 / §10.
  - [ ] Update markup env builder to discover `templates/macros/*.html` and inject `{% import … as <stem> %}` automatically.
  - [ ] Add `tests/fixtures/11_macros/` with a `youtube` macro and a Markdown doc using `{{ youtube::embed(id="abc") }}` in its body.
  - [ ] Verify: `cargo test` confirms the macro output appears in the rendered HTML and survives Markdown rendering intact.

- [ ] Phase 12: `new` scaffolding + built-in RSS & Sitemap generators — `knead new <name>` initializes a site skeleton; the scaffold ships starter RSS and sitemap generator templates that the end-to-end build actually produces.
  - [ ] Add `new` subcommand to the CLI; embed the scaffold templates with `include_str!`.
  - [ ] Scaffold contents: `config.yaml`, `content/index.md`, `templates/base.html`, `generators/rss.xml`, `generators/sitemap.xml`, `static/.gitkeep`.
  - [ ] RSS generator: query latest N posts ordered by `date desc`, output `/feed.xml`.
  - [ ] Sitemap generator: query everything, `order: 9999`, output `/sitemap.xml`.
  - [ ] Add `tests/fixtures/12_scaffold/` whose expected output includes `feed.xml` and `sitemap.xml` produced by the scaffold's own generators (no test-only generator copies).
  - [ ] Verify: `cargo test` confirms the scaffolded site builds and produces both feed.xml and sitemap.xml with the right entries.
  - [ ] Verify: manual `knead new demo && cd demo && knead build` works.

- [ ] Phase 13: `watch` command — `knead watch` runs a full rebuild whenever any source dir changes (content, templates, generators, data, static, config.yaml).
  - [ ] Add `notify` to `Cargo.toml`.
  - [ ] Add `src/watch.rs::run` — debounce file events (~150ms) and invoke `build::run` on each batch; log build duration and errors without exiting.
  - [ ] Wire `watch` subcommand in `clap`.
  - [ ] Verify: manual smoke — touch a `.md` file, see rebuild log within ~250ms and updated `dist/` content.
  - [ ] Verify: errors during a rebuild do not kill the watcher; subsequent fixes recover.
