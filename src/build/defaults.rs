//! The defaults phase. After collections are classified (frontmatter-only,
//! pre-markup) and before markup runs, fill each collection's members with the
//! default frontmatter declared for that collection in `config.yaml`'s
//! `defaults:` block.
//!
//! Defaults reference collections by name, so the set a default applies to is
//! exactly that collection's evaluated membership. A doc's own frontmatter always
//! wins (fill-missing); when several `defaults:` entries cover the same doc, the
//! later config entry wins. Running before markup means a defaulted taxonomy
//! field (e.g. `tags`) is seen by the post-markup taxonomy classification.

use crate::config::Config;
use crate::doc_index::DocIndex;
use anyhow::Result;
use serde_yaml_ng::Mapping;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn run(config: &Config, index: &mut DocIndex) -> Result<()> {
    if config.defaults.is_empty() {
        return Ok(());
    }

    // Combine defaults of every collection it belongs to, in config
    // order so a later `defaults:` entry overwrites an earlier one on overlap.
    let mut per_doc: HashMap<PathBuf, Mapping> = HashMap::new();
    for (name, frontmatter) in &config.defaults {
        let ids: Vec<PathBuf> = index
            .get_collection(name)
            .map(|doc| doc.id_path.clone())
            .collect();
        for id in ids {
            let combined = per_doc.entry(id).or_default();
            for (k, v) in frontmatter {
                combined.insert(k.clone(), v.clone());
            }
        }
    }

    // Apply each doc's combined defaults, then re-derive its metadata fields from
    // the merged frontmatter. Order across docs is irrelevant — each doc's
    // combined map is fully built first, and the merge is fill-missing.
    for (id, combined) in &per_doc {
        if let Some(doc) = index.doc_mut(id) {
            doc.apply_defaults(combined);
            doc.uplift_frontmatter(&config.taxonomies);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;
    use crate::query::Query;
    use globset::GlobBuilder;
    use std::path::Path;

    fn glob(pattern: &str) -> Query {
        Query {
            path: Some(
                GlobBuilder::new(pattern)
                    .literal_separator(true)
                    .build()
                    .unwrap(),
            ),
            ..Query::default()
        }
    }

    fn doc(id_path: &str) -> Doc {
        Doc {
            id_path: PathBuf::from(id_path),
            output_path: PathBuf::from(id_path).with_extension("html"),
            ..Default::default()
        }
    }

    fn frontmatter(s: &str) -> Mapping {
        serde_yaml_ng::from_str(s).unwrap()
    }

    /// Build a config + classified index, run defaults, return the index.
    fn build(
        collections: Vec<(&str, Query)>,
        defaults: Vec<(&str, &str)>,
        taxonomies: Vec<String>,
        docs: Vec<Doc>,
    ) -> DocIndex {
        let config = Config {
            collections: collections
                .into_iter()
                .map(|(n, q)| (n.to_string(), q))
                .collect(),
            defaults: defaults
                .into_iter()
                .map(|(n, fm)| (n.to_string(), frontmatter(fm)))
                .collect(),
            taxonomies,
            ..Config::default()
        };
        let mut index = DocIndex::new();
        for d in docs {
            index.insert(d);
        }
        crate::build::classify::collections(&config, &mut index);
        run(&config, &mut index).unwrap();
        // Mirror the pipeline: taxonomies are classified after defaults.
        crate::build::classify::taxonomies(&config, &mut index);
        index
    }

    #[test]
    fn later_defaults_entry_wins_on_overlap() {
        // `a.md` is in both collections; the later `defaults:` entry (`special`)
        // wins for `template`.
        let index = build(
            vec![("all", glob("*.md")), ("special", glob("a.md"))],
            vec![
                ("all", "template: base.html"),
                ("special", "template: special.html"),
            ],
            vec!["tags".to_string()],
            vec![doc("a.md"), doc("b.md")],
        );
        assert_eq!(
            index.doc(Path::new("a.md")).unwrap().template.as_deref(),
            Some("special.html")
        );
        // `b.md` only matched `all`.
        assert_eq!(
            index.doc(Path::new("b.md")).unwrap().template.as_deref(),
            Some("base.html")
        );
    }

    #[test]
    fn defaulted_tag_shows_up_in_taxonomy() {
        // A default that sets `tags:` is applied pre-(taxonomy)-classification, so
        // the term lands in the `tags` taxonomy.
        let index = build(
            vec![("posts", glob("posts/*.md"))],
            vec![("posts", "tags: [Rust]")],
            vec!["tags".to_string()],
            vec![doc("posts/p.md")],
        );
        let tags = index.get_taxonomy("tags").unwrap();
        assert_eq!(
            tags.get("rust").unwrap(),
            &vec![PathBuf::from("posts/p.md")]
        );
    }

    #[test]
    fn own_frontmatter_beats_default() {
        let mut d = doc("posts/p.md");
        d.data = frontmatter("template: own.html");
        d.template = Some("own.html".into());
        let index = build(
            vec![("posts", glob("posts/*.md"))],
            vec![("posts", "template: default.html")],
            vec!["tags".to_string()],
            vec![d],
        );
        assert_eq!(
            index
                .doc(Path::new("posts/p.md"))
                .unwrap()
                .template
                .as_deref(),
            Some("own.html")
        );
    }
}
