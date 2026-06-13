//! Tera adapter for the whole corpus. `all()` returns the always-present
//! [`all`](crate::config::ALL) collection from the frozen [`DocIndex`] shared
//! across the template phase. That collection is guaranteed to exist: when a
//! site/theme does not declare its own `all` under `collections:`,
//! `Config::load_with_theme` injects one with the default [`Query`], so `all()`
//! lists every doc in date-desc order out of the box. It is the zero-config
//! escape hatch for "list everything" that needs no `collections:` entry.
//!
//! Because it is backed by a collection, a site that *does* declare `all:` under
//! `collections:` reorders, omits, or filters what `all()` returns.
//!
//! It still takes no arguments by design: ordering, limiting, and filtering are
//! what collections are for (`collection(name=...)` with an `order_by`/`sort`/
//! `omit` definition — including redefining `all` itself), or what the array
//! filters do at render time (`omit_docs`, `dirtree`, `filter_in_dir`, `slice`).
//! So any kwarg is rejected up front — a typo'd `all(limt=5)` fails loudly
//! rather than silently ignoring the cap.

use crate::config::ALL;
use crate::doc_index::DocIndex;
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Tera, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "all",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            check_no_args(args)?;
            let docs: Vec<&crate::doc::Doc> = index.get_collection(ALL).collect();
            tera::to_value(docs).map_err(tera::Error::from)
        },
    );
}

/// `all()` takes no arguments. Reject any kwarg so an attempt at a
/// `limit=`/`order_by=`/`omit=` (which `all` deliberately does not support)
/// fails loudly rather than being silently dropped — define a `collection`, or
/// pipe the result through array filters, instead.
fn check_no_args(args: &HashMap<String, Value>) -> tera::Result<()> {
    if let Some(key) = args.keys().next() {
        return Err(tera::Error::msg(format!(
            "all: unexpected argument `{}` (all() takes no arguments — define a \
             collection to order/limit/filter, or pipe through filters like \
             omit_docs, dirtree, or slice)",
            key
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_no_args_accepts_empty() {
        assert!(check_no_args(&HashMap::new()).is_ok());
    }

    #[test]
    fn check_no_args_rejects_any_kwarg() {
        let mut args = HashMap::new();
        args.insert("limit".to_string(), Value::from(5u64));
        assert!(check_no_args(&args).is_err());
    }
}
