/// Escape the five XML/HTML special characters so a string can be safely
/// embedded in attribute values or element text.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_all_five() {
        assert_eq!(escape("a & b < c > d \" e ' f"), "a &amp; b &lt; c &gt; d &quot; e &#39; f");
    }

    #[test]
    fn leaves_plain_text_untouched() {
        assert_eq!(escape("hello world"), "hello world");
    }

    #[test]
    fn preserves_non_ascii() {
        assert_eq!(escape("café · 你好"), "café · 你好");
    }

    #[test]
    fn empty_string() {
        assert_eq!(escape(""), "");
    }
}
