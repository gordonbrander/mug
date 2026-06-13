# Deployment

`italic build` produces plain static files in `public/` — any static host
works. The recipes below cover the common ones.

Two settings matter everywhere:

```yaml
site:
  url: https://example.com   # so feeds and social tags get absolute URLs
  base_path: ""              # set when hosting under a subpath
```

## GitHub Pages

For a **project site** served at `username.github.io/repo/`, set
`base_path: /repo` and use the [URL filters](permalinks.md#urls-site-url-and-base-path)
in your templates. For a **user site** (`username.github.io`) or a custom
domain, leave `base_path` empty.

`.github/workflows/deploy.yml`:

```yaml
name: Deploy
on:
  push:
    branches: [main]
permissions:
  contents: read
  pages: write
  id-token: write
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install italic
      - run: italic build
      - uses: actions/upload-pages-artifact@v3
        with:
          path: public
  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

(Caching `~/.cargo` or installing a prebuilt binary will speed up the install
step considerably.)

## Netlify

`netlify.toml` at the repo root:

```toml
[build]
command = "cargo install italic && italic build"
publish = "public"
```

Netlify's build image includes the Rust toolchain via its standard tooling;
alternatively build in CI and deploy the `public/` folder with
`netlify deploy --prod --dir=public`.

## Cloudflare Pages

In the Pages project settings:

- **Build command**: `cargo install italic && italic build`
- **Build output directory**: `public`

Or skip remote builds entirely: build locally or in CI and push with
`wrangler pages deploy public`.

## Plain server (rsync)

```sh
italic build
rsync -avz --delete public/ user@server:/var/www/example.com/
```

`--delete` keeps the server in sync with removals; run `italic clean` first if
you've changed permalinks and want a guaranteed-fresh build.

## Staging with drafts

A staging environment can include drafts:

```sh
italic build --drafts
```

See [Drafts](drafts.md).

## See also

- [CLI reference](../reference/cli.md)
- [Permalinks](permalinks.md) — `base_path` and URL filters
