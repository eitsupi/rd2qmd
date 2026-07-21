//! Inline-level node writers.

use super::{Writer, escape_link_title, format_link_destination};

impl<'a> Writer<'a> {
    pub(super) fn write_emphasis(&mut self, e: &crate::mdast::Emphasis) {
        self.output.push('_');
        for child in &e.children {
            self.write_node(child);
        }
        self.output.push('_');
    }

    pub(super) fn write_strong(&mut self, s: &crate::mdast::Strong) {
        self.output.push_str("**");
        for child in &s.children {
            self.write_node(child);
        }
        self.output.push_str("**");
    }

    pub(super) fn write_inline_code(&mut self, c: &crate::mdast::InlineCode) {
        let formatted = crate::format_inline_code(&c.value, self.output.ends_with('`'));
        self.output.push_str(&formatted);
    }

    pub(super) fn write_break(&mut self) {
        self.output.push_str("  \n");
        self.at_line_start = true;
    }

    pub(super) fn write_link(&mut self, l: &crate::mdast::Link) {
        // CommonMark autolinks require an absolute URI (with scheme) and
        // no whitespace or angle brackets
        if l.title.is_none()
            && matches!(l.children.as_slice(),
                [crate::mdast::Node::Text(t)] if t.value == l.url)
            && is_absolute_uri(&l.url)
        {
            self.output.push('<');
            self.output.push_str(&l.url);
            self.output.push('>');
            return;
        }
        self.output.push('[');
        for child in &l.children {
            self.write_node(child);
        }
        self.output.push_str("](");
        self.output.push_str(&format_link_destination(&l.url));
        if let Some(title) = &l.title {
            self.output.push_str(" \"");
            self.output.push_str(&escape_link_title(title));
            self.output.push('"');
        }
        self.output.push(')');
    }

    pub(super) fn write_image(&mut self, img: &crate::mdast::Image) {
        self.output.push_str("![");
        self.output.push_str(&img.alt);
        self.output.push_str("](");
        self.output.push_str(&format_link_destination(&img.url));
        if let Some(title) = &img.title {
            self.output.push_str(" \"");
            self.output.push_str(&escape_link_title(title));
            self.output.push('"');
        }
        self.output.push(')');
    }

    pub(super) fn write_math(&mut self, m: &crate::mdast::Math) {
        self.ensure_newline();
        self.output.push_str("$$\n");
        self.output.push_str(&m.value);
        if !m.value.ends_with('\n') {
            self.output.push('\n');
        }
        self.output.push_str("$$\n");
        self.at_line_start = true;
    }

    pub(super) fn write_inline_math(&mut self, m: &crate::mdast::InlineMath) {
        self.output.push('$');
        self.output.push_str(&m.value);
        self.output.push('$');
    }
}

/// Whether `s` is a valid CommonMark autolink URI: a scheme
/// (`[A-Za-z][A-Za-z0-9+.-]{1,31}`) followed by `:` and any characters
/// other than ASCII control, space, `<`, or `>`.
fn is_absolute_uri(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    (2..=32).contains(&scheme.len())
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        && !rest
            .chars()
            .any(|c| c.is_ascii_control() || matches!(c, ' ' | '<' | '>'))
}
