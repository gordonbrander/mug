# Plan: knead — Static Site Generator

- Spec: spec.md

## Overview

**Goals**: Build a zero-config, Rust-based static site generator whose every site-specific behavior lives in `config.yaml`, frontmatter, and Tera templates. The pipeline reads `content/`, renders bodies, generates virtual pages from `generators/`, and templates everything against a single in-memory index that supports path/tag/backlink queries. Targets the surface defined in spec.md §3–§13.

**Approach**: Land thin, demoable vertical slices, but **front-load the type skeleton** so every phase is shaping a known structure rather than inventing one. Phase 1 lands the core types (`Doc`, `Config`, `Index`, the four phase function signatures) *and* a minimal end-to-end build through all four phases over a single Markdown file. Subsequent phases either populate fields the `Doc`/`Index` already carry or fill in a phase whose signature is already in place. The binary stays runnable and verifiable after every phase. Functional core (pure `Doc` operations, query evaluation, permalink expansion) sits behind an imperative shell (filesystem walking, Tera registry, output writing). Integration fixtures under `tests/fixtures/` drive verification — every phase ships at least one fixture site whose expected output is asserted.

**Tech**: Rust 2024 edition, single binary crate. Standard library plus the dependencies enumerated below.

### Dependencies

Pulled in up front so the type skeleton in Phase 1 can be modeled against real crate types (e.g. `chrono::DateTime`, `serde_yaml_ng::Mapping`) rather than placeholders that get swapped later. The list is intentionally small — the spec's "small, predictable surface area" goal applies to our dependency graph too.

**Phase 1 (skeleton):**

- **clap** (with `derive` feature) — CLI parsing. Derive-macro subcommands (`build`, `watch`, `new`) keep `main.rs` declarative. Mature, low-churn; the obvious default.
- **walkdir** — recursive directory traversal for `content/`, `generators/`, `data/`, `static/`. Simple, no-config, handles symlinks correctly. Plain `std::fs::read_dir` recursion would also work but `walkdir` is a one-liner.
- **pulldown-cmark** — CommonMark parser. Rust-native, pull-based, fast, the de-facto choice. Used in the markup phase to render `.md` bodies into `doc.content`. Enable the `html` feature.
- **serde** (with `derive`) + **serde_yaml_ng** — frontmatter, `config.yaml`, and `data/` cascade. We pick **`serde_yaml_ng`** rather than the original `serde_yaml` because the upstream crate is unmaintained (deprecated by its author); `serde_yaml_ng` is the community fork with the same API. `Doc.data` is typed as `serde_yaml_ng::Mapping` so all frontmatter survives uplift verbatim.
- **chrono** (with `serde` feature) — `DateTime<Utc>` for `doc.date` / `doc.updated`. Lands in Phase 1 because those fields exist on `Doc` from the start, even though Phase 2 is the first time they carry real values. Used again in Phase 6 for `:yyyy/:mm/:dd` permalink expansion.
- **anyhow** — application-level error plumbing. We are a binary, not a library, so structured error enums (via `thiserror`) are not worth the ceremony; `anyhow::Result<T>` with `.context(...)` annotations is enough.

**Phase 3 (templates):**

- **tera** — Jinja-like runtime template engine. A *runtime* engine is mandatory here (users supply their own templates, so compile-time engines like `askama` are unsuitable). Tera natively supports macros (the spec's shortcode mechanism, Phase 11), custom filters (`query`, `backlinks`, `permalink`), and template inheritance. We construct two `Tera` instances — one with the restricted filter set for the markup phase, one with the full set for the template phase — so the spec §11 restricted-markup invariant is enforced by construction.

**Phase 6 (permalinks):**

- **slug** — sluggify document stems and wikilink targets. Used for the `:slug` permalink variable and again in Phase 9 for wikilink resolution. Tiny crate, no transitive deps worth noting.

**Phase 7 (query):**

- **globset** — compile glob patterns once and match many paths cheaply. Backs the `path:` operator in queries. Picked over `glob` because `globset` is built for repeated matches against a large set of paths, which is exactly the query-evaluation pattern.

**Phase 13 (watch):**

- **notify** (v6+, with the default debouncer) — cross-platform file watcher. Coalesces filesystem events on macOS (FSEvents), Linux (inotify), and Windows so we get one rebuild per editor save, not three.

**Dev-dependencies (Phase 1 onward):**

- **insta** — snapshot testing for fixture-driven integration tests. Each phase ships a fixture site under `tests/fixtures/<phase>/` whose built output is compared against a stored snapshot, with `cargo insta review` for accepted updates. Avoids hand-maintaining `expected/` trees as the renderer evolves. Could alternatively roll our own string-diff helper, but `insta`'s review workflow pays for itself by Phase 3.

**Explicitly not pulling in:**

- *gray_matter* / dedicated frontmatter crates — `---`-delimited splitting is ~20 lines of code; not worth the dependency.
- *thiserror* — we have no public library API; `anyhow` is sufficient.
- *regex* — wikilink scanning is a simple state machine; a hand-rolled scanner avoids a heavy transitive dep.
- *tokio* / async runtime — the build is CPU-bound and inherently sequential across phases; `rayon` may be worth considering inside the markup phase later but is not in v1.

## TODO

- [ ] Phase 1: Type skeleton + four-phase pipeline — land the core data types (`Doc`, `Config`, `Index`) and stubbed signatures for all four phases (`read`, `markup`, `generate`, `template`), wired into a `build` command that renders one Markdown file end-to-end through every phase. Most `Doc` fields exist but stay at defaults; most phases are near-passthrough. The shape is right, the contents grow later.
  - [ ] Add `clap` (`derive`), `walkdir`, `pulldown-cmark` (`html`), `serde` (`derive`), `serde_yaml_ng`, `chrono` (`serde`), `anyhow` to `[dependencies]`; add `insta` to `[dev-dependencies]`.
  - [ ] Add `src/config.rs::Config` with `content_dir`, `output_dir`, `templates_dir`, `static_dir`, `data_dir`, `generators_dir`; `Default` impl returns spec §3 names; no file load yet.
  - [ ] Add `src/doc.rs::Doc` with **all fields from spec §5** present from day one: `id_path: PathBuf`, `output_path: PathBuf`, `template: Option<String>`, `title: String`, `content: String`, `tags: Vec<String>`, `date: DateTime<Utc>`, `updated: DateTime<Utc>`, `data: serde_yaml_ng::Mapping`. Add a `Doc::from_body(id_path, body) -> Doc` constructor that fills defaults so Phase 1 can use it without frontmatter.
  - [ ] Add `src/index.rs::Index` with `docs: Vec<Doc>`, `by_path: BTreeMap<PathBuf, usize>`; reserve (commented) slots for `by_tag` and `backlinks` to be populated later. Provide `Index::insert(doc)` and `Index::get(&id_path)`.
  - [ ] Add `src/phases/mod.rs` re-exporting `read`, `markup`, `generate`, `template` modules.
  - [ ] Add `src/phases/read.rs::run(&Config) -> Index` — walks `content_dir`, dispatches `.md` only, builds `Doc` via `Doc::from_body`, inserts into `Index`. (Other extensions handled in later phases.)
  - [ ] Add `src/phases/markup.rs::run(&mut Index)` — for each doc, render `.md` body via `pulldown-cmark` into `doc.content`. (Tera, frontmatter handling, and per-type branching land in later phases.)
  - [ ] Add `src/phases/generate.rs::run(&Config, &mut Index)` — empty stub (no-op) with a doc comment marking Phase 8.
  - [ ] Add `src/phases/template.rs::run(&Config, &mut Index)` — passthrough; `doc.content` is the final output. (Tera lands in Phase 3.)
  - [ ] Add `src/phases/write.rs::run(&Config, &Index)` — writes each doc's `content` to `output_dir.join(doc.output_path)`. Default `output_path` in Phase 1 mirrors `id_path` with `.html` extension.
  - [ ] Replace `src/main.rs` hello-world with a `clap` dispatcher; `build` calls `read → markup → generate → template → write` in order.
  - [ ] Add `tests/fixtures/01_skeleton/` with one `content/hello.md` and the expected `dist/hello.html`.
  - [ ] Add `tests/build.rs` integration harness (helper `run_build(fixture_name)`) that runs the pipeline against a fixture and diffs the output tree against `expected/`.
  - [ ] Verify: `cargo build` succeeds with no `dead_code` warnings on the new types (use `#[allow(dead_code)]` where genuinely deferred).
  - [ ] Verify: `cargo test` passes the `01_skeleton` fixture.
  - [ ] Verify: manual `cargo run -- build` in `tests/fixtures/01_skeleton/` produces `dist/hello.html`.

- [ ] Phase 2: Frontmatter uplift — populate the `Doc` fields that Phase 1 left at defaults by parsing the `---`-delimited YAML block.
  - [ ] Add `src/frontmatter.rs::split` returning `(Option<&str>, &str)` for the YAML block and body, plus `parse(&str) -> serde_yaml_ng::Mapping`.
  - [ ] Add `Doc::from_source(id_path, source, fs_meta)` — splits frontmatter, uplifts `title`, `template`, `tags`, `date`, `updated` per spec §5 fallback rules (date: frontmatter → created → modified; updated: frontmatter → modified), stashes the full frontmatter map in `doc.data`.
  - [ ] Update `phases/read.rs` to use `Doc::from_source` instead of `Doc::from_body`.
  - [ ] Add unit tests for `frontmatter::split` (empty, missing closing `---`, body-only, CRLF) and for `Doc::from_source` (each fallback rule).
  - [ ] Add `tests/fixtures/02_frontmatter/` with a doc declaring `title`, `tags`, `date`; expected output is the rendered body only (still no template wrapping).
  - [ ] Verify: `cargo test` passes.
  - [ ] Verify: manual build of `02_frontmatter` strips the frontmatter from output and exposes uplifted fields (visible later once templates land).

- [ ] Phase 3: Tera template phase — fill in the `phases/template.rs` stub with real Tera rendering; load `templates/`; `doc.template` selects a template. Markup phase also runs Tera over bodies (restricted: no `query`/`backlinks` registered yet).
  - [ ] Add `tera` to `Cargo.toml`.
  - [ ] Add `src/tera_env.rs` with `build_markup_env(&Config) -> Tera` (restricted) and `build_template_env(&Config) -> Tera` (full). Both load `templates/**/*.html` and `templates/**/*.xml`. Filter registration is the only difference; the registry is the seam later phases extend.
  - [ ] Update `phases/markup.rs`: for `.md` bodies run restricted Tera over the body string before Markdown render.
  - [ ] Update `phases/template.rs`: for each `Doc`, render its `doc.template` (default: passthrough) against a context `{ doc, page: { content: doc.content } }`.
  - [ ] Add `tests/fixtures/03_templates/` with `templates/base.html`, a doc declaring `template: base.html`, and expected wrapped output.
  - [ ] Verify: `cargo test` passes; a negative test confirms the restricted env errors when a body calls `query`/`backlinks`.
  - [ ] Verify: manual build of `03_templates` produces the templated HTML.

- [ ] Phase 4: HTML and YAML input types — extend the `read` phase dispatch and the markup branch table. The `Doc` shape is unchanged; this slice just teaches the existing pipeline to produce a `Doc` from `.html` and `.yaml` sources.
  - [ ] Add a `DocKind { Markdown, Html, Yaml }` enum on `Doc` (or compute it from extension at markup time — pick one and document the choice in `doc.rs`).
  - [ ] Extend `phases/read.rs` to recognize `.html` and `.yaml` and build `Doc`s via `Doc::from_source` (Markdown/HTML) or a new `Doc::from_yaml(id_path, source)` (YAML — whole file is frontmatter; `content` field becomes `doc.content`).
  - [ ] Extend `phases/markup.rs`: HTML path runs restricted Tera over body, skips Markdown; YAML path runs restricted Tera on the `content` field (already pulled out at read time).
  - [ ] Add `tests/fixtures/04_input_types/` with one of each type and expected outputs.
  - [ ] Verify: `cargo test` passes for all three input types in the same fixture.
  - [ ] Verify: manual build produces an HTML file per source regardless of input type.

- [ ] Phase 5: Static dir + `config.yaml` + `data/` cascade — extend `Config` with loaded `site` data, introduce a `SiteData` carrier for `config` + `data/`, and pass it to the template context.
  - [ ] Add `src/config.rs::Config::load(path)` — parse `config.yaml` into `Config` (merged over defaults); zero-config still works if file is absent.
  - [ ] Add `src/site_data.rs::SiteData { config: serde_yaml_ng::Mapping, data: serde_yaml_ng::Mapping }`; `load(&Config) -> SiteData` reads `data/**/*.yaml` into a nested map keyed by relative path components (e.g. `data/site/nav.yaml` → `data.site.nav`).
  - [ ] Thread `SiteData` through the pipeline (signature change on `template::run` and `markup::run`); inject `site` and `data` into the Tera context.
  - [ ] Add `src/phases/static_copy.rs::run(&Config)` — recursive copy from `static/` to `dist/`; call after `write`.
  - [ ] Add `tests/fixtures/05_cascade/` with `static/robots.txt`, `config.yaml` declaring `site.title`, `data/menu.yaml`, a template referencing all three.
  - [ ] Verify: `cargo test` passes.
  - [ ] Verify: manual build copies `static/` and renders config + data values into the templated output.

- [ ] Phase 6: Permalinks — fill in the `Doc::output_path` slot with real `permalink` expansion. `Doc` already carries `output_path` from Phase 1; this slice replaces the default-mirror computation with `permalink:` expansion of `:slug`, `:yyyy`, `:mm`, `:dd`.
  - [ ] Add `slug` to `Cargo.toml`.
  - [ ] Add `src/permalink.rs::expand(pattern: &str, doc: &Doc) -> PathBuf` supporting the variable set from spec §5.1; trailing `/` means write `index.html`.
  - [ ] Default behavior (no `permalink`) still mirrors `id_path` (existing Phase 1 behavior, now factored as `permalink::default_for(&id_path)`).
  - [ ] Move `output_path` resolution from `write::run` into the end of `read::run` so markup can rely on it (sets up Phase 9 wikilink resolution).
  - [ ] Add `tests/fixtures/06_permalinks/` with a post declaring `permalink: /blog/:yyyy/:slug/` and the expected `dist/blog/2025/hello/index.html`.
  - [ ] Add unit tests for `permalink::expand` (date components, trailing slash, default fallback).
  - [ ] Verify: `cargo test` passes.

- [ ] Phase 7: `Index` populated + `query` filter — populate the `by_tag` slot already reserved on `Index` in Phase 1, add a `Query` type and evaluator, register the filter on the template env only.
  - [ ] Add `globset` to `Cargo.toml`.
  - [ ] Promote `Index.by_tag: HashMap<String, Vec<usize>>` from reserved-but-unused to populated; fill it at the end of `read::run`.
  - [ ] Add `src/query.rs::Query { path: Option<Glob>, tag: Option<String>, order_by: OrderKey, sort: SortDir }` plus a parser for the compact form `path:<glob>, tag:<t> order_by:<f> sort:<d>` and a struct form callable from Tera kwargs.
  - [ ] Add `src/query.rs::evaluate(query: &Query, index: &Index) -> Vec<&Doc>`.
  - [ ] Register `query` as a Tera function on the template env only; the markup env stays restricted.
  - [ ] Add `tests/fixtures/07_query/` with three posts and an `index.html` template listing them via `{{ query(...) }}`.
  - [ ] Verify: `cargo test` passes and verifies ordering by `date desc`.
  - [ ] Verify: negative test asserts calling `query` from a Markdown body errors during markup.

- [ ] Phase 8: Generators + pagination — fill in the `phases/generate.rs` stub left in Phase 1. Introduce `Generator` and `Pagination` types; emit `Doc` descriptors that join the existing `Index`.
  - [ ] Add `src/generator.rs::Generator { id_path, query, per_page: Option<usize>, permalink: String, template: Option<String>, order: i32, body: String }`.
  - [ ] Add `src/generator.rs::Pagination { current: usize, total: usize, prev_url: Option<String>, next_url: Option<String>, items: Vec<DocId> }`.
  - [ ] Implement `phases/generate.rs::run` — read each file in `generators/`, parse frontmatter into a `Generator`, sort by `order` ascending, evaluate the query for each, chunk by `per_page`, build a `Doc` per page with `pagination` stashed in `doc.data`.
  - [ ] After fan-out, run the markup step over each new descriptor and insert into the index so high-`order` generators (e.g. sitemap at `9999`) observe everything emitted before them.
  - [ ] Extend `permalink::expand` to support `:page` for pagination patterns.
  - [ ] Add `tests/fixtures/08_generators/` with five posts and a generator producing paginated index pages (2 per page).
  - [ ] Verify: `cargo test` confirms three pages emitted with correct prev/next URLs and item slices.
  - [ ] Verify: a sitemap-like generator with `order: 9999` sees a doc emitted by an earlier generator (asserted in fixture).

- [ ] Phase 9: Wikilinks + `permalink` filter — promote `Index.backlinks` from reserved slot to populated; resolve `[[Wiki Link]]` / `[[Wiki Link|Display]]` in markup; register `permalink` filter on both envs.
  - [ ] Promote `Index.backlinks: HashMap<usize, Vec<usize>>` from reserved-but-unused to populated.
  - [ ] Add `src/wikilink.rs::expand(body, doc, &mut Index) -> String` — scan-and-replace pass run before Markdown render; emits `<a href="…">…</a>` with the resolved URL and records a backlink edge.
  - [ ] Implement resolution: slug the target, walk `doc.id_path` parent chain looking for a matching stem; first match wins.
  - [ ] Register a `permalink` Tera filter on both envs that takes an `id_path` and returns the `output_path`.
  - [ ] Add `tests/fixtures/09_wikilinks/` with nested docs exercising same-dir, parent-dir, and root resolution, plus a `|Display` case.
  - [ ] Verify: `cargo test` confirms each wikilink resolves to the correct URL.
  - [ ] Verify: unresolved wikilinks produce a clear warning and fall back to plain text (assert in fixture).

- [ ] Phase 10: `backlinks` filter — register `backlinks` on the template env only; accepts `order_by` (`title` | `created` | `updated`) and `sort`.
  - [ ] Add `backlinks` filter implementation in `src/tera_env.rs` reading the index's backlink graph.
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
