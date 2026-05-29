use crate::build::generator::{Generator, Pagination};
use crate::build::markup;
use crate::config::Config;
use crate::doc::{Doc, DocMeta};
use crate::index::Index;
use crate::permalink;
use crate::query;
use crate::site_data::SiteData;
use crate::tera_env::build_markup_env;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_yaml_ng::Value;
use std::fs;
use std::sync::Arc;
use walkdir::WalkDir;

pub fn run(config: &Config, site_data: &SiteData, index: &mut Index) -> Result<()> {
    if !config.generators_dir.exists() {
        return Ok(());
    }

    let mut generators: Vec<Generator> = Vec::new();
    for entry in WalkDir::new(&config.generators_dir) {
        let entry =
            entry.with_context(|| format!("walking {}", config.generators_dir.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let id_path = path
            .strip_prefix(&config.generators_dir)
            .with_context(|| format!("stripping prefix from {}", path.display()))?
            .to_path_buf();
        let source =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let g = Generator::parse(id_path, &source)
            .with_context(|| format!("parsing generator {}", path.display()))?;
        generators.push(g);
    }

    // Lower weight runs first. A sitemap with weight 9999 observes everything
    // emitted by earlier generators (spec §9.1).
    generators.sort_by_key(|g| g.weight);

    // Frozen `DocMeta` view of the post-authored-markup index for wikilink
    // resolution and the URL filters inside generator bodies.
    let snapshot: Arc<Vec<DocMeta>> = Arc::new(index.docs.iter().map(DocMeta::from).collect());
    let mut markup_env = build_markup_env(config, snapshot.clone())?;

    for g in generators {
        let matched: Vec<&Doc> = query::evaluate(&g.query, &index.docs);

        // per_page=0 or unset → single page with every item.
        let per_page = g
            .per_page
            .filter(|n| *n > 0)
            .unwrap_or(matched.len().max(1));
        let total_pages = if matched.is_empty() {
            1
        } else {
            matched.len().div_ceil(per_page)
        };

        // Buffer emitted docs and insert them after the page loop: `matched`
        // borrows `index.docs`, so the index can't be mutated until that borrow
        // ends. Flushing per-generator (before the next generator's query)
        // preserves spec §9.1 cross-generator visibility.
        let mut emitted: Vec<Doc> = Vec::new();
        for page_idx in 0..total_pages {
            let page = page_idx + 1;
            let start = page_idx * per_page;
            let end = ((page_idx + 1) * per_page).min(matched.len());
            let items: Vec<Doc> = matched[start..end].iter().map(|d| (*d).clone()).collect();

            let output_path = permalink::expand(&g.permalink, &g.id_path, &epoch(), Some(page));
            let prev_url = (page > 1).then(|| {
                permalink::to_url(&permalink::expand(
                    &g.permalink,
                    &g.id_path,
                    &epoch(),
                    Some(page - 1),
                ))
            });
            let next_url = (page < total_pages).then(|| {
                permalink::to_url(&permalink::expand(
                    &g.permalink,
                    &g.id_path,
                    &epoch(),
                    Some(page + 1),
                ))
            });

            let pagination = Pagination {
                current: page,
                total: total_pages,
                prev_url,
                next_url,
                items,
            };

            let mut data = g.data.clone();
            data.insert(
                Value::String("pagination".into()),
                serde_yaml_ng::to_value(&pagination).context("serializing pagination context")?,
            );

            let mut doc = Doc {
                id_path: output_path.clone(),
                output_path,
                template: g.template.clone(),
                title: String::new(),
                summary: String::new(),
                content: g.body.clone(),
                tags: Vec::new(),
                date: epoch(),
                updated: epoch(),
                data,
                links: Vec::new(),
            };

            markup::render(&mut markup_env, site_data, &mut doc)?;
            emitted.push(doc);
        }
        for doc in emitted {
            index.insert(doc);
        }
    }

    Ok(())
}

fn epoch() -> DateTime<Utc> {
    DateTime::<Utc>::UNIX_EPOCH
}
