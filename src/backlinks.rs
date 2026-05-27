use crate::doc::Doc;
use crate::query::{OrderKey, SortDir};
use std::path::Path;

pub struct Backlinks {
    pub order_by: OrderKey,
    pub sort: SortDir,
}

impl Default for Backlinks {
    fn default() -> Self {
        Self {
            order_by: OrderKey::Created,
            sort: SortDir::Desc,
        }
    }
}

/// Linear scan over `docs`: collect any whose `outlinks` contain `target`,
/// then sort. No persistent inverted graph — the spec §11 "fully populated
/// before listing" invariant is what makes this correct.
pub fn list_backlinks<'a>(docs: &'a [Doc], target: &Path, b: &Backlinks) -> Vec<&'a Doc> {
    let mut results: Vec<&Doc> = docs
        .iter()
        .filter(|d| d.outlinks.iter().any(|o| o == target))
        .collect();

    results.sort_by(|a, b2| {
        let cmp = match b.order_by {
            OrderKey::Title => a.title.cmp(&b2.title),
            OrderKey::Created => a.date.cmp(&b2.date),
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

    fn doc(id_path: &str, title: &str, date: &str, outlinks: &[&str]) -> Doc {
        let mut d = Doc::default();
        d.id_path = PathBuf::from(id_path);
        d.title = title.to_string();
        d.date = at(date);
        d.updated = at(date);
        d.outlinks = outlinks.iter().map(PathBuf::from).collect();
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
    fn list_backlinks_default_order_is_created_desc() {
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
        };
        let results = list_backlinks(&docs, Path::new("target.md"), &b);
        let titles: Vec<&str> = results.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn list_backlinks_does_not_include_self_unless_self_outlinks() {
        // Source linking to itself should still appear in its own backlinks.
        let docs = vec![doc("a.md", "A", "2025-01-01", &["a.md"])];
        let b = Backlinks::default();
        let results = list_backlinks(&docs, Path::new("a.md"), &b);
        assert_eq!(results.len(), 1);
    }
}
