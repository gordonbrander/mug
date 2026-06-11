# Contributing

Italic is AGPL-licensed Rust. Issues and pull requests are welcome at
[github.com/gordonbrander/italic](https://github.com/gordonbrander/italic).

## Getting set up

```sh
git clone https://github.com/gordonbrander/italic.git
cd italic
cargo build
cargo test
```

## Repo tour

```
src/
  main.rs          # CLI (clap) — the six subcommands
  config.rs        # config.yaml parsing, defaults, theme merging
  doc.rs           # the Doc type; frontmatter uplift
  permalink.rs     # permalink patterns, pagination URLs
  build.rs         # the pipeline driver — start here
  build/           # one module per stage: read, classify, defaults,
                   #   markup (incl. wikilink resolution), archive,
                   #   template, write, static_copy
  tera_env.rs      # Tera environment assembly
  tera_env/        # one module per custom function/filter
scaffold/          # site skeleton emitted by `italic new`
tests/
  build.rs         # fixture-driven integration tests
  fixtures/        # numbered end-to-end sites (01_skeleton … 10_backlinks)
docs/              # this documentation; internal notes in docs/notes/
```

The build pipeline's stage order and data contracts are documented in the
module comment at the top of `src/build.rs`, and at user level in
[The build pipeline](concepts/build-pipeline.md).

## Tests

- Unit tests live alongside the code (`#[cfg(test)]` modules).
- Integration tests in `tests/build.rs` run each `tests/fixtures/NN_*` site
  through a full build and compare the output tree against the fixture's
  `expected/` directory. Adding a feature? Add or extend a fixture — they
  double as living examples for the documentation.

Run everything with `cargo test`.

## Conventions

- Modern Rust module style: `foo.rs` with a sibling `foo/` directory, not
  `foo/mod.rs`.
- Errors are loud: unknown config keys and bad references fail the build with
  a pointer, never a silent ignore. Match that spirit in new features.
- User-visible changes that touch behavior get an update to the relevant page
  under `docs/`.
