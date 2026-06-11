# Italic

A static site generator for [digital gardens](https://maggieappleton.com/garden-history).

- Built for thinkers: wikilinks, backlinks, custom collections, related notes, custom taxonomies, and more.
- Batteries included: One binary with everything you need. Zero config required.
- Fast: Build thousands of pages in < 1s. Written in Rust with an embarrassingly parallel rendering pipeline.

## Features

Italic makes it easy to publish a digital garden from your
[Obsidian Vault](https://obsidian.md/), or any other folder full of Markdown.

- Markdown extensions: compatible with [GitHub-flavored Markdown](https://github.github.com/gfm/) and [Obsidian Markdown](https://obsidian.md/help/syntax)
- Wikilinks: fuzzy link matching using the same algorithm as Obsidian
- Backlinks: see what links into a page
- Hashtags: auto-appended to tags and stripped from output
- Related: surface related pages, scored over taxonomies and the link graph

Plus everything else you'd expect from a static site generator, and a few extras:

- Blog-aware: publish multiple blogs from the same site
- Custom collections: a powerful query system collects pages into any grouping you want
- Multiple taxonomies: categorize by tag, series, publication, phase of the moon — no problem
- Themes, powerful [Tera](https://keats.github.io/tera/docs) templates, shortcodes
- Archives, drafts, RSS feeds, sitemaps, and more

## Install

```sh
cargo install italic
```

This puts `italic` on your `PATH` (typically `~/.cargo/bin/italic`).

## Quick start

```sh
italic new my-site
cd my-site
echo '# Hello, world' > content/index.md
italic serve
```

Congrats! You have a website at <http://localhost:3000>.

To dress it up, grab a starter theme:

```sh
git clone --depth 1 https://github.com/gordonbrander/italic_themes.git themes/
```

```yaml
# config.yaml
theme: "themes/obsidian"
```

Then `italic build` outputs plain static files to `public/`, ready for any
host. The [quickstart](docs/getting-started/quickstart.md) covers all of this
in more detail.

## Documentation

Full documentation lives in [docs/](docs/index.md):

- **[Quickstart](docs/getting-started/quickstart.md)** — zero to website in four commands
- **[Tutorial](docs/getting-started/tutorial.md)** — publish your Obsidian vault, with backlinks, tags, and feeds
- **Concepts** — [project layout](docs/concepts/project-layout.md), [content model](docs/concepts/content-model.md), [the build pipeline](docs/concepts/build-pipeline.md)
- **Guides** — [wikilinks](docs/guides/wikilinks.md), [related pages](docs/guides/related.md), [collections](docs/guides/collections.md), [taxonomies](docs/guides/taxonomies.md), [templates](docs/guides/templates.md), [archives & feeds](docs/guides/archives.md), [themes](docs/guides/themes.md), [deployment](docs/guides/deployment.md), [migration](docs/guides/migration.md), and more
- **Reference** — [CLI](docs/reference/cli.md), [configuration](docs/reference/config.md), [frontmatter](docs/reference/frontmatter.md), [templates](docs/reference/templates.md)

## License

AGPL — see [LICENSE-AGPL](LICENSE-AGPL).
