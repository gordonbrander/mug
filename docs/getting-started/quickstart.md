# Quickstart

From nothing to a served website in four commands.

## 1. Create a site

```sh
italic new my-site
cd my-site
```

This scaffolds an empty starter site, including a fully commented
`config.yaml` that shows every available setting (all optional — italic is
zero-config).

## 2. Add a page

```sh
echo '# Hello, world' > content/index.md
```

Any Markdown file in `content/` becomes a page.

## 3. Serve it

```sh
italic serve
```

Congrats! You have a website at <http://localhost:3000>, rebuilding live on
every change.

## 4. Add a theme (optional)

The default site is unstyled. Grab the starter themes and pick one:

```sh
git clone --depth 1 https://github.com/gordonbrander/italic_themes.git themes/
```

```yaml
# config.yaml
theme: "themes/obsidian"
```

Optionally add the theme's demo content to see it dressed:

```sh
italic scaffold
```

## 5. Build for publishing

```sh
italic build
```

The site lands in `public/` — plain static files, ready for any host. See
[Deployment](../guides/deployment.md).

## Where next

- [Tutorial: publish your Obsidian vault](tutorial.md) — the full garden
  workflow: wikilinks, backlinks, tags, feeds.
- [Project layout](../concepts/project-layout.md) — what the scaffolded
  directories mean.
- [Configuration reference](../reference/config.md) — every `config.yaml` key.
