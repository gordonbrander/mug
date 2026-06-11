# Installation

Italic ships as a single binary with everything included — no runtime, plugin
directory, or node_modules.

## With cargo

```sh
cargo install italic
```

This puts `italic` on your `PATH` (typically `~/.cargo/bin/italic`). If you
don't have the Rust toolchain, install it first from
[rustup.rs](https://rustup.rs).

## From source

```sh
git clone https://github.com/gordonbrander/italic.git
cd italic
cargo install --path .
```

## Verify

```sh
italic --help
```

You should see the six subcommands (`build`, `watch`, `serve`, `new`,
`scaffold`, `clean`).

## Upgrade / uninstall

```sh
cargo install italic          # installs the latest published version
cargo uninstall italic
```

## Next

Head to the [Quickstart](quickstart.md) — you're five minutes from a website.
