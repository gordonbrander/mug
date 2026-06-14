# Related docs: weighted term-overlap over taxonomies and links

## Context

`italic` has no "related content" feature — given a doc, surface the docs most
like it. `TODO.md` wants one, and the building blocks are already in place:
`Doc.terms` ([`doc.rs:31`](../../src/doc.rs)) holds each doc's taxonomy
memberships, `Doc.links` ([`doc.rs:35`](../../src/doc.rs)) holds its outbound
link targets, and `DocIndex` already inverts terms into `taxonomy → term → docs`
via [`get_taxonomy`](../../src/doc_index.rs) (`doc_index.rs:150`).

The interesting design question is *what to relate by*. Hugo's default relates
by keywords and **date** (it buckets dates into year-strings and treats the
bucket as a keyword). For us, date-proximity is somewhat less important. The
signals that matter most are **shared taxonomy terms** (two notes tagged
`#phenomenology`) and **links** (two notes that cite the same source, or link to
each other).

This proposal adds a `related(doc)` function built on a single mechanism —
**weighted term-overlap over an inverted index** — and shows that *links fold
into that same mechanism* rather than needing a separate code path. Date clustering
is optional, and could be folded in if it uses the same machinery.

The guiding constraint is Italic's identity (`README.md`): *"does one thing well."*
The whole feature is one scoring loop over indices that already exist.

---

## The core idea: relatedness as weighted term-overlap

Model every relationship as **shared terms in a namespace**. A doc belongs under
a set of terms in each namespace; two docs are related in proportion to how many
terms they share, weighted by namespace:

```
score(post, other) = Σ_namespace  weight[namespace] × |terms(post, ns) ∩ terms(other, ns)|
```

- **`tags`, `categories`, …** — namespace = a taxonomy; terms = the doc's term
  slugs in that taxonomy (`Doc.terms[name]`). This is the obvious case.
- **`links`** — namespace = the link graph; terms = **`{doc's own id_path} ∪
  {doc's outbound link targets}`**.

That second line is the load-bearing trick. Treating a link as "just another
term" means one symmetric overlap captures all three link relationships at once:

| Relationship | Why it shows up as a shared term |
|---|---|
| **Co-citation** — A and B both link to C | both have term `C` (a shared outbound target) |
| **Forward link** — A links to B | B contributes its *own* id as a term, so A and B share term `B` |
| **Backlink** — B links to A | symmetric to the above; same shared term `A` |

No separate backlink scan, no adjacency special-case — links become one more
weighted namespace in the same loop as tags.

### This needs almost no new index code

`DocIndex` already has a generic inverted-index builder,
[`define_taxonomy`](../../src/doc_index.rs) (`doc_index.rs:129`), whose `term_fn`
returns the terms a doc belongs under. The `links` namespace is just one more
call to it:

```rust
// in define_taxonomies / a new define_link_index, alongside the real taxonomies
self.define_taxonomy("links", |doc| {
    let mut terms: Vec<String> =
        doc.links.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    terms.push(doc.id_path.to_string_lossy().into_owned()); // own id
    terms
});
```

The result keys on id_path strings, so `get_taxonomy("links")` returns
`term(id_path) → docs that are it or link to it` — exactly the candidate set the
scorer needs, retrieved the same way as `get_taxonomy("tags")`.

---

## Algorithm

```
related(post, weights, limit?):
  scores = {}                                  # id_path -> f64
  for (namespace, weight) in weights:
    for term in terms_of(post, namespace):     # links: {self id} ∪ outbound
      for other in get_taxonomy(namespace)[term]:   # reuse the cached index
        if other == post.id_path: continue     # never relate a doc to itself
        scores[other] += weight × idf(term)     # idf = 1.0 in phase 1
  rank = scores.into_iter().collect::<Vec<_>>()
  rank.sort_by(score desc, then date desc, then id_path asc)
  truncate to `limit` if given
```

Two properties this must hold, both load-bearing in this codebase:

- **Exclude self.** A doc shares every one of its own terms (including its own
  id in `links`) with itself, so without this guard it tops its own list. This
  is the same concern [`list_backlinks`](../../src/backlinks.rs) handles with
  `omit` (`backlinks.rs:31`).
- **Total ordering.** Scores tie constantly. Accumulate in a map, but the final
  sort must break ties deterministically — `score desc, date desc, id_path asc`
  — to match the order `define_taxonomy` (`doc_index.rs:140`) and `list_backlinks`
  (`backlinks.rs:50`) already produce. This matters because the build is
  parallelized with Rayon; nondeterministic output would be a regression.

Drafts need no handling: the read phase already drops them before the index is
built (`doc.rs:22`), so `get_taxonomy` only ever sees published docs.

---

## Two design decisions

These are the real forks. Both are resolved here with a recommendation and
deferred refinement, in the spirit of the phased `assets.md` proposal.

### 1. Should a direct link outweigh co-citation?

The single `links` namespace above weighs a forward/backlink **the same** as a
shared citation, because the shared term carries no record of which kind it was.
If a direct A↔B link should count for more than "A and B both cite C", split
into two namespaces with independent weights:

- `links` — term-set = outbound targets only → overlap = **co-citation**.
- a direct-adjacency pass — candidates `post` links to, plus its backlinks —
  each scored at a heavier weight.

**Recommendation:** ship the **single equal-weight `links` namespace** in
Phase 1. It is genuinely one mechanism and the per-namespace `weight` knob
already lets authors dial links up or down against tags. Promote to the split
only if equal-weighting proves too coarse in practice.

### 2. Flat counts or rarity-weighted (IDF)?

Flat `+=1` says sharing `#rust` (200 docs) is as meaningful as sharing
`#phenomenology` (3 docs). It usually is not — in a personal vault the long-tail
tags carry the signal. Because the inverted index already knows each term's
document frequency (`get_taxonomy(ns)[term].len()`), down-weighting common terms
by `1/df` or `1/ln(1+df)` is nearly free.

**Recommendation:** ship **flat counts** in Phase 1 (`idf(term) = 1.0`), but
factor the scorer so `idf` is a pluggable function. Turn on IDF in Phase 2 once
there is a real vault to tune against. Keeping it a seam costs nothing now and
avoids a rewrite later.

---

## Integration

**`src/related.rs`** — new module, mirroring `backlinks.rs`: a `Related` options
struct (`weights: Vec<(String, f64)>`, `limit: Option<usize>`) with a `Default`,
and a `fn related<'a>(index: &'a DocIndex, post: &Path, opts: &Related) ->
Vec<&'a Doc>` that runs the algorithm above. Computed **on demand** like
backlinks — no persistent structure beyond the inverted indices `DocIndex`
already caches.

**`src/doc_index.rs`** — add the `links` pseudo-taxonomy. Either extend
`define_taxonomies` to always also build `"links"`, or add a sibling
`define_link_index`. Built in the classify phase alongside the real taxonomies,
honoring the spec §11 "index fully populated before listing" invariant that
`backlinks.rs:24` relies on.

**`src/config.rs`** — a `related:` block parsed like `collections:`/`taxonomies:`
(`config.rs:100`, `config.rs:104`), into a new `#[serde(skip)]` field on
`Config`. Lets a site set per-namespace weights and a default limit:

```yaml
related:
  weights:
    tags: 3.0
    categories: 1.0
    links: 2.0
  limit: 5
```

Sensible defaults when the block is absent: equal weight on every declared
taxonomy plus `links`, no limit. `tags` is the built-in taxonomy
(`taxonomy.rs:18`), so the zero-config case still relates by tags + links.

**`src/tera_env/related.rs`** — new Tera adapter, modeled exactly on
[`tera_env/backlinks.rs`](../../src/tera_env/backlinks.rs): `register(env,
Arc<DocIndex>)`, kwargs validated against a `KNOWN_KEYS` allowlist (`limit`,
maybe `omit`), serialize the result. Template-env only (spec §11 forbids
index-listing filters in the markup env). Usage:

```jinja
{% for doc in page.id_path | related(limit=5) %}
  <a href="{{ doc.output_path | url }}">{{ doc.title }}</a>
{% endfor %}
```

---

## Phasing

**Phase 1 (ship this):** single `links` namespace (equal-weight, capturing
forward/back/co-citation), flat term counts with an `idf` seam stubbed to `1.0`,
`related:` config block, `related()` function, Tera filter. One mechanism, one
scoring loop, leans entirely on the existing inverted index.

**Phase 2 (deferred, tune against a real vault):** turn on IDF rarity-weighting;
optionally split `links` into weighted direct-vs-co-citation if equal-weighting
proves too coarse. Both are local changes to the scorer behind seams Phase 1
leaves in place — no structural rework.

---

## Testing

Unit tests in `related.rs`, following the `backlinks.rs` table-style helpers
(`backlinks.rs:70`):

- shared tags rank by overlap count; weights reorder results.
- a doc never appears in its own related list (the self-id guard, incl. a
  self-linking doc — cf. `backlinks.rs:177`).
- co-citation: A and B both link to C → related, even with no shared tags.
- forward/backlink: A links B → each appears in the other's list.
- deterministic tie-break: equal scores fall back to date desc then id_path
  (cf. `collection_order_is_stable_across_equal_sort_keys`, `doc_index.rs:217`).
- `limit` truncates; absent limit returns all.
- empty case: a doc with no terms and no links relates to nothing.

## Out of scope

- **Date-proximity / recency clustering** — Hugo's default; deliberately dropped
  as the wrong signal for a knowledge vault (see Context).
- **Body-text / semantic similarity** — relatedness here is metadata-driven term
  overlap only, exactly as the inverted index supports. No NLP, no embeddings.
- **A persistent relatedness cache** — computed on demand like backlinks;
  revisit only if profiling shows it dominates build time on a real vault.
