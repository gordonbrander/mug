use crate::index::Index;
use anyhow::Result;
use pulldown_cmark::{Parser, html};

pub fn run(index: &mut Index) -> Result<()> {
    for doc in &mut index.docs {
        let parser = Parser::new(&doc.content);
        let mut html_out = String::new();
        html::push_html(&mut html_out, parser);
        doc.content = html_out;
    }
    Ok(())
}
