//! Options + rarity-weighting seam for the `related` doc-similarity query. The
//! scorer itself lives on [`DocIndex::related`](crate::doc_index::DocIndex::related)
//! because it reads the inverted taxonomy and link indices; this module owns the
//! `Related` options struct and the `idf` seam that shapes scoring.

use std::path::PathBuf;

/// The reserved namespace name for the link graph. As a `related` weight key it
/// scores by the **whole, undirected wikilink graph** — not just backlinks. Two
/// pages are linked-related if any of these hold (the relation is symmetric: if
/// it relates A to B, it relates B to A):
/// - a page this page **links to** (its outbound targets), or
/// - a page that **links to** this page (its backlinks), or
/// - a page that **links to the same target** this page does (a shared outbound
///   reference — *bibliographic coupling*).
///
/// So this is broader than the [`backlinks`](crate::backlinks) filter, which is
/// the incoming direction only. Because the graph lives in its own index (not
/// the taxonomy map), a site may still *declare* a taxonomy literally named
/// `links`; it simply can't be addressed through `related` weights, where the
/// name is reserved for the graph.
pub const LINKS: &str = "links";

/// Per-query options for `related`.
#[derive(Debug, Clone, Default)]
pub struct Related {
    /// `(namespace, weight)` pairs. A namespace is either a taxonomy name —
    /// whose shared term slugs drive overlap — or the special [`LINKS`]
    /// namespace. Config order; an unknown namespace contributes nothing.
    pub weights: Vec<(String, f64)>,
    /// Docs to exclude from the result (besides the query doc itself, which is
    /// always excluded). Applied before `limit`.
    pub omit: Vec<PathBuf>,
    pub limit: Option<usize>,
}

/// Corpus-relative inverse-document-frequency weight for a shared term that
/// `df` of the corpus's `n` docs carry. A term shared by few docs outweighs a
/// common one, and rarity is measured *relative to corpus size* — so the same
/// `df` counts for more as the vault grows (a tag on 3 of 4 notes is common; on
/// 3 of 4000 it is rare). The `1.0 +` smooths the universal-term case (`df == n`
/// → `ln 2`) so a term on every doc still breaks ties rather than vanishing.
/// In practice `2 <= df <= n` (a term only `post` carries is shared with no one,
/// so it never scores), keeping the argument `> 1` and the result positive.
pub fn idf(df: usize, n: usize) -> f64 {
    (1.0 + n as f64 / df as f64).ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idf_rewards_rarer_terms() {
        // Within one corpus, a term shared by fewer docs weighs strictly more.
        assert!(idf(2, 100) > idf(5, 100));
        assert!(idf(5, 100) > idf(50, 100));
    }

    #[test]
    fn idf_is_corpus_relative() {
        // The same df is rarer — and so weighs more — in a larger corpus.
        assert!(idf(3, 4000) > idf(3, 4));
    }

    #[test]
    fn idf_universal_term_stays_positive() {
        // A term on every doc (df == n) still contributes (ln 2), so it can break
        // ties rather than vanishing.
        let universal = idf(10, 10);
        assert!(universal > 0.0);
        assert!((universal - 2.0_f64.ln()).abs() < 1e-12);
    }
}
