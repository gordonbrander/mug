# Tutorial: publish your Obsidian vault

This walkthrough takes a folder of Markdown notes — an Obsidian vault, or any
notes directory — and turns it into a published digital garden with working
wikilinks, backlinks, tags, a blog, and an RSS feed. Allow twenty minutes.

You'll need italic [installed](installation.md).

## 1. Start from your notes

```sh
italic new my-garden
cd my-garden
```

Copy your notes into `content/` (a few sample notes work fine too):

```sh
cp -R ~/vault/* content/
```

Italic doesn't care how the folder is organized — keep your existing
structure. Serve it:

```sh
italic serve
```

Open <http://localhost:3000>. Your notes are already pages: `[[wikilinks]]`
resolve with Obsidian's fuzzy matching, and every resolved link is feeding a
backlink graph we'll render shortly. Leave `serve` running; everything below
reloads live.

## 2. Add a base layout

Unstyled HTML is honest but bleak. Create `templates/base.html`:

```html
<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>{{ page.title }} | {{ site.title }}</title>
</head>
<body>
  <main>
    <h1>{{ page.title }}</h1>
    {{ page.content | safe }}
  </main>

  <aside>
    <h2>Linked from</h2>
    <ul>
    {% for src in page.id_path | backlinks %}
      <li><a href="{{ src.id_path | link }}">{{ src.title }}</a></li>
    {% endfor %}
    </ul>

    <h2>Related</h2>
    <ul>
    {% for doc in page.id_path | related(limit=5) %}
      <li><a href="{{ doc.id_path | link }}">{{ doc.title }}</a></li>
    {% endfor %}
    </ul>
  </aside>
</body>
</html>
```

And tell italic to use it for everything. In `config.yaml`:

```yaml
site:
  title: My Garden

collections:
  notes:
    path: "**/*.md"

defaults:
  notes:
    template: base.html
```

The `notes` collection matches every Markdown file, and its `defaults:` entry
assigns the layout — no per-file frontmatter needed. Each note now shows its
**backlinks** (who links here) and **related pages** (scored across shared
tags and the link graph).

## 3. Turn on tags and hashtags

In `config.yaml`:

```yaml
taxonomies:
  - tags

hashtags: true
```

Now `tags: [...]` frontmatter *and* inline `#hashtags` in note bodies both
feed the `tags` taxonomy — and sharpen the related-pages scoring. Give a few
notes some tags.

## 4. Add tag pages

Create `archives/tags.html` to generate one page per tag:

```yaml
---
kind: taxonomy
taxonomy: tags
permalink: /tags/:term/
template: base.html
---
{% for post in pagination.items %}
  <p><a href="{{ post.id_path | permalink }}">{{ post.title }}</a></p>
{% endfor %}
```

Visit `/tags/<some-tag>/` to see it.

## 5. Add a blog beside the garden

A garden and a blog can share a site. Put dated posts in `content/posts/` and
add to `config.yaml`:

```yaml
collections:
  notes:
    path: "**/*.md"
  posts:
    path: "posts/*.md"
    order_by: date
    sort: desc

defaults:
  notes:
    template: base.html
  posts:
    template: base.html
    permalink: /blog/:yyyy/:slug/
```

And a reverse-chronological archive at `/blog/`, `archives/blog.html`:

```yaml
---
kind: collection
collection: posts
permalink: /blog/
per_page: 10
template: base.html
---
{% for post in pagination.items %}
  <p>
    <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
    <time>{{ post.date | date(format="%Y-%m-%d") }}</time>
  </p>
{% endfor %}
```

## 6. Add an RSS feed

`archives/feed.xml`:

```yaml
---
kind: collection
collection: posts
permalink: /feed.xml
limit: 20
---
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0">
<channel>
  <title>{{ site.title }}</title>
  <link>{{ "" | absolute_url }}</link>
  <description>{{ site.description }}</description>
  {% for post in pagination.items %}
  <item>
    <title>{{ post.title }}</title>
    <link>{{ post.id_path | permalink }}</link>
    <pubDate>{{ post.date | date(format="%a, %d %b %Y %H:%M:%S +0000") }}</pubDate>
  </item>
  {% endfor %}
</channel>
</rss>
```

For the feed's absolute URLs, set your site's origin in `config.yaml`:

```yaml
site:
  title: My Garden
  url: https://example.com
```

## 7. Publish

```sh
italic build
```

Everything lands in `public/` as plain static files. Drafts
(`draft: true` notes) are excluded automatically. Pick a host and ship it —
recipes for GitHub Pages, Netlify, Cloudflare Pages, and rsync are in the
[Deployment guide](../guides/deployment.md).

## Where next

- Style it: wrap the layout in CSS, or adopt a [theme](../guides/themes.md).
- [Permalinks](../guides/permalinks.md) — clean URLs for everything.
- [Macros](../guides/macros.md) — video embeds and shortcodes in your notes.
- [Migration guide](../guides/migration.md) — coming from Jekyll, Hugo, or Quartz.
