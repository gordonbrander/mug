use crate::doc::Doc;
use crate::query::{OrderKey, SortDir};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct Backlinks {
    pub order_by: OrderKey,
    pub sort: SortDir,
    pub omit: Vec<PathBuf>,
}

impl Default for Backlinks {
    fn default() -> Self {
        Self {
            order_by: OrderKey::Date,
            sort: SortDir::Desc,
            omit: Vec::new(),
        }
    }
}

/// Linear scan over `docs`: collect any whose `links` contain `target`,
/// then sort. No persistent inverted graph — the spec §11 "fully populated
/// before listing" invariant is what makes this correct.
pub fn list_backlinks<'a>(docs: &'a [Doc], target: &Path, b: &Backlinks) -> Vec<&'a Doc> {
    let omit: HashSet<&Path> = b.omit.iter().map(PathBuf::as_path).collect();

    let mut results: Vec<&Doc> = docs
        .iter()
        .filter(|d| !omit.contains(d.id_path.as_path()) && d.links.iter().any(|o| o == target))
        .collect();

    results.sort_by(|a, b2| {
        let cmp = match b.order_by {
            OrderKey::Title => a.title.cmp(&b2.title),
            OrderKey::Date => a.date.cmp(&b2.date),
            OrderKey::Updated => a.updated.cmp(&b2.updated),
        };
        match b.sort {
            SortDir::Asc => cmp,
            SortDir::Desc => cmp.reverse(),
        }
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};
    use std::path::PathBuf;

    fn at(date: &str) -> DateTime<Utc> {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
    }

    fn doc(id_path: &str, title: &str, date: &str, links: &[&str]) -> Doc {
        let mut d = Doc::default();
        d.id_path = PathBuf::from(id_path);
        d.title = title.to_string();
        d.date = at(date);
        d.updated = at(date);
        d.links = links.iter().map(PathBuf::from).collect();
        d
    }

    #[test]
    fn list_backlinks_no_backlinks_returns_empty() {
        let docs = vec![doc("a.md", "A", "2025-01-01", &[])];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("b.md"), &b);
        assert!(results.is_empty());
    }

    #[test]
    fn list_backlinks_finds_single_backlink() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01", &["b.md"]),
            doc("b.md", "B", "2025-01-02", &[]),
        ];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("b.md"), &b);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "A");
    }

    #[test]
    fn list_backlinks_finds_multiple_backlinks() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01", &["b.md"]),
            doc("c.md", "C", "2025-01-03", &["b.md", "other.md"]),
            doc("d.md", "D", "2025-01-02", &["other.md"]),
        ];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("b.md"), &b);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn list_backlinks_default_order_is_date_desc() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01", &["target.md"]),
            doc("b.md", "B", "2025-02-01", &["target.md"]),
            doc("c.md", "C", "2025-03-01", &["target.md"]),
        ];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("target.md"), &b);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["C", "B", "A"]);
    }

    #[test]
    fn list_backlinks_order_by_title_asc() {
        let docs = vec![
            doc("a.md", "Charlie", "2025-01-01", &["target.md"]),
            doc("b.md", "Alpha", "2025-01-02", &["target.md"]),
            doc("c.md", "Bravo", "2025-01-03", &["target.md"]),
        ];
        let b = Backlinks {
            order_by: OrderKey::Title,
            sort: SortDir::Asc,
            ..Default::default()
        };
        let results = list_backlinks(&docs, Path::new("target.md"), &b);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn list_backlinks_excludes_omitted_docs() {
        let docs = vec![
            doc("a.md", "A", "2025-01-01", &["target.md"]),
            doc("b.md", "B", "2025-02-01", &["target.md"]),
            doc("c.md", "C", "2025-03-01", &["target.md"]),
        ];
        let b = Backlinks {
            omit: vec![PathBuf::from("b.md")],
            ..Default::default()
        };
        let results = list_backlinks(&docs, Path::new("target.md"), &b);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["C", "A"]);
    }

    #[test]
    fn list_backlinks_omit_self_drops_self_link() {
        // The common case: a page links to itself but should not list itself
        // among its own backlinks.
        let docs = vec![
            doc("a.md", "A", "2025-01-01", &["target.md"]),
            doc("target.md", "Target", "2025-01-02", &["target.md"]),
        ];
        let b = Backlinks {
            omit: vec![PathBuf::from("target.md")],
            ..Default::default()
        };
        let results = list_backlinks(&docs, Path::new("target.md"), &b);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["A"]);
    }

    #[test]
    fn list_backlinks_does_not_include_self_unless_self_links() {
        // Source linking to itself should still appear in its own backlinks.
        let docs = vec![doc("a.md", "A", "2025-01-01", &["a.md"])];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("a.md"), &b);
        assert_eq!(results.len(), 1);
    }
}
