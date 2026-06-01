//! `markdown` filter — renders its string input through comrak. Registered on
//! both envs so `{% filter markdown %}…{% endfilter %}` (and `value | markdown`)
//! work in Markdown bodies and HTML/XML templates alike. Marked safe so its
//! HTML output is not autoescaped in `.html`/`.xml` templates.

use comrak::plugins::syntect::SyntectAdapter;
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Filter, Tera, Value};

struct MarkdownFilter {
    options: comrak::Options<'static>,
    syntect: Arc<SyntectAdapter>,
}

impl Filter for MarkdownFilter {
    fn filter(&self, value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let input = value
            .as_str()
            .ok_or_else(|| tera::Error::msg("markdown filter: input must be a string"))?;
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, input, &self.options);
        let mut plugins = comrak::options::Plugins::default();
        plugins.render.codefence_syntax_highlighter = Some(self.syntect.as_ref());
        let mut out = String::new();
        comrak::format_html_with_plugins(root, &self.options, &mut out, &plugins)
            .map_err(|e| tera::Error::msg(format!("markdown filter: comrak render failed: {e}")))?;
        tera::to_value(out).map_err(tera::Error::from)
    }

    fn is_safe(&self) -> bool {
        true
    }
}

pub fn register(env: &mut Tera, options: comrak::Options<'static>, syntect: Arc<SyntectAdapter>) {
    env.register_filter("markdown", MarkdownFilter { options, syntect });
}
