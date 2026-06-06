//! Dev tool: generate a large corpus of interlinked Markdown pages for
//! load/perf-testing the `italic` build pipeline.
//!
//! Reads a YAML fixture of titles, tags, a date range, and prose blocks (see
//! `examples/looking_glass.yaml`), then emits `--num-pages` Markdown files with
//! YAML frontmatter, randomized dates, random `tags:`, and `[[wikilinks]]` to
//! other generated pages so backlinks/related/taxonomies get a real graph.
//!
//! Randomness is a hand-rolled seeded PRNG, so a given `--seed` always produces
//! a byte-identical corpus (no `rand` dependency).
//!
//! Usage:
//! ```text
//! cargo run --example gen_fixtures -- --num-pages 5000
//! cargo run --example gen_fixtures -- --num-pages 50000 --out ./bench_site/content --seed 7
//! ```
//! Pair it with a scaffolded site: `cargo run -- new bench_site`, then build
//! with `cd bench_site && cargo run --manifest-path ../Cargo.toml -- build`.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Generate bulk Markdown fixtures for load-testing italic")]
struct Args {
    /// Number of pages to generate
    #[arg(long)]
    num_pages: usize,

    /// Output directory for generated `.md` files
    #[arg(long, default_value = "./bench_site/content")]
    out: PathBuf,

    /// YAML fixture providing titles, tags, date range, and prose blocks
    #[arg(long, default_value = "examples/looking_glass.yaml")]
    fixture: PathBuf,

    /// PRNG seed — same seed produces a byte-identical corpus
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Minimum prose blocks per page
    #[arg(long, default_value_t = 3)]
    min_blocks: usize,

    /// Maximum prose blocks per page
    #[arg(long, default_value_t = 7)]
    max_blocks: usize,

    /// Maximum `[[wikilinks]]` to other pages per page
    #[arg(long, default_value_t = 3)]
    links: usize,

    /// Maximum frontmatter tags per page
    #[arg(long, default_value_t = 3)]
    tags: usize,

    /// Remove the output directory before generating
    #[arg(long)]
    clean: bool,
}

#[derive(Deserialize)]
struct Fixture {
    titles: Vec<String>,
    tags: Vec<String>,
    date_range: DateRange,
    blocks: Vec<String>,
}

#[derive(Deserialize)]
struct DateRange {
    start: String,
    end: String,
}

/// splitmix64 — a tiny, fast, well-distributed PRNG. Deterministic for a seed.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `[lo, hi]` (inclusive). Returns `lo` if `hi <= lo`.
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() as usize) % (hi - lo + 1)
    }

    fn choose<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0, items.len() - 1)]
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let raw = fs::read_to_string(&args.fixture)
        .with_context(|| format!("reading fixture {}", args.fixture.display()))?;
    let fixture: Fixture = serde_yaml_ng::from_str(&raw).context("parsing fixture YAML")?;

    anyhow::ensure!(!fixture.titles.is_empty(), "fixture has no titles");
    anyhow::ensure!(!fixture.blocks.is_empty(), "fixture has no blocks");
    anyhow::ensure!(
        args.min_blocks >= 1 && args.max_blocks >= args.min_blocks,
        "need 1 <= min_blocks <= max_blocks"
    );

    let start = NaiveDate::parse_from_str(fixture.date_range.start.trim(), "%Y-%m-%d")
        .context("parsing date_range.start (want YYYY-MM-DD)")?;
    let end = NaiveDate::parse_from_str(fixture.date_range.end.trim(), "%Y-%m-%d")
        .context("parsing date_range.end (want YYYY-MM-DD)")?;
    let span_days = (end - start).num_days().max(0) as usize;

    let mut rng = Rng(args.seed);

    // Pre-compute every page's title up front so wikilinks can target real
    // pages. A per-base counter ("Humpty Dumpty 7") keeps titles — and thus
    // slugs and filenames — unique.
    let mut counters: HashMap<&str, usize> = HashMap::new();
    let titles: Vec<String> = (0..args.num_pages)
        .map(|_| {
            let base = rng.choose(&fixture.titles);
            let n = counters.entry(base.as_str()).or_insert(0);
            *n += 1;
            format!("{base} {n}")
        })
        .collect();

    if args.clean && args.out.exists() {
        fs::remove_dir_all(&args.out)
            .with_context(|| format!("cleaning {}", args.out.display()))?;
    }
    fs::create_dir_all(&args.out).with_context(|| format!("creating {}", args.out.display()))?;

    for i in 0..args.num_pages {
        let title = &titles[i];

        // Body: a random run of prose blocks.
        let n_blocks = rng.range(args.min_blocks, args.max_blocks);
        let mut body: Vec<String> = (0..n_blocks)
            .map(|_| rng.choose(&fixture.blocks).clone())
            .collect();

        // Wikilinks to other generated pages (skip self).
        let n_links = rng.range(0, args.links);
        let mut links = Vec::new();
        for _ in 0..n_links {
            if args.num_pages < 2 {
                break;
            }
            let mut j = rng.range(0, args.num_pages - 1);
            if j == i {
                j = (j + 1) % args.num_pages;
            }
            links.push(format!("- [[{}]]", titles[j]));
        }
        if !links.is_empty() {
            body.push(format!("## See also\n\n{}", links.join("\n")));
        }

        // Frontmatter tags.
        let n_tags = rng.range(0, args.tags.min(fixture.tags.len()));
        let mut chosen: Vec<&String> = Vec::new();
        for _ in 0..n_tags {
            let t = rng.choose(&fixture.tags);
            if !chosen.contains(&t) {
                chosen.push(t);
            }
        }

        // Date within the configured range.
        let date = start + chrono::Duration::days(rng.range(0, span_days) as i64);

        let mut fm = String::new();
        fm.push_str("---\n");
        fm.push_str(&format!("title: {}\n", yaml_scalar(title)));
        fm.push_str(&format!("date: {}\n", date.format("%Y-%m-%d")));
        if !chosen.is_empty() {
            let list = chosen
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            fm.push_str(&format!("tags: [{list}]\n"));
        }
        fm.push_str("---\n\n");

        let contents = format!("{fm}{}\n", body.join("\n\n"));
        let path = args.out.join(format!("{}.md", slug::slugify(title)));
        fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
    }

    println!(
        "Generated {} pages in {} (seed {}).",
        args.num_pages,
        args.out.display(),
        args.seed
    );
    Ok(())
}

/// Quote a frontmatter title only when YAML would otherwise mis-parse it.
fn yaml_scalar(s: &str) -> String {
    let needs_quote = s.is_empty()
        || s.starts_with([
            '"', '\'', '[', '{', '#', '*', '&', '!', '|', '>', '%', '@', '`',
        ])
        || s.contains(": ")
        || s.ends_with(':')
        || s.contains(" #");
    if needs_quote {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}
