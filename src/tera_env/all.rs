//! Tera adapter for the whole corpus. `all()` returns every doc in the frozen
//! [`DocIndex`] shared across the template phase, in the default listing order
//! (`id_path` — the `BTreeMap`'s natural order, stable across runs and machines).
//! It is the zero-config escape hatch for "list everything" that needs no
//! `collections:` entry in `config.yaml`.
//!
//! It takes no arguments by design: ordering, limiting, and filtering are what
//! collections are for (`collection(name=...)` with an `order_by`/`sort`/`omit`
//! definition), or what the array filters do at render time (`omit_docs`,
//! `dirtree`, `filter_in_dir`, `slice`). So any kwarg is rejected up front — a
//! typo'd `all(limt=5)` fails loudly rather than silently ignoring the cap.

use crate::doc_index::DocIndex;
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Tera, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "all",
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            check_no_args(args)?;
            let docs: Vec<&crate::doc::Doc> = index.docs().collect();
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
