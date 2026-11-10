//! Inline-level node writers.

use super::{Writer, escape_link_title, format_link_destination};

impl<'a> Writer<'a> {
    /// Write a sequence of inline nodes.
    ///
    /// Each node is told the first character that will follow it, which
    /// `write_emphasis` needs to pick a delimiter.
    ///
    /// `trailing` is the character the parent writes right after the last
    /// child (for example its closing delimiter), or `None` at the end of a
    /// block.
    pub(super) fn write_inlines(&mut self, nodes: &[crate::mdast::Node], trailing: Option<char>) {
        for (i, node) in nodes.iter().enumerate() {
            self.next_char = nodes[i + 1..].iter().find_map(first_char).or(trailing);
            self.write_node(node);
        }
        self.next_char = None;
    }

    /// Write emphasis, using `_` unless that would sit inside a word.
    ///
    /// CommonMark does not recognize `_` as an emphasis delimiter next to an
    /// alphanumeric character (`before_em_after` is literal text), while `*`
    /// works there. `*` is not used by default because it would make
    /// `\strong{\emph{x}}` render as `***x***`, which Quarto rejects.
    ///
    /// `*` is also never placed directly next to another `*`, since runs such
    /// as `**b*x***` do not parse. When an emphasis is intraword right at such
    /// a boundary, neither delimiter works; `_` is kept there, matching the
    /// previous output. Such rare cases may still be rejected by Quarto 2.
    pub(super) fn write_emphasis(&mut self, e: &crate::mdast::Emphasis) {
        let after = self.next_char;
        let before = self.output.chars().next_back();
        let intraword =
            before.is_some_and(char::is_alphanumeric) || after.is_some_and(char::is_alphanumeric);
        let star_adjacent = before == Some('*') || after == Some('*');
        let delim = if intraword && !star_adjacent {
            '*'
        } else {
            '_'
        };
        self.output.push(delim);
        self.write_inlines(&e.children, Some(delim));
        self.output.push(delim);
        self.next_char = after;
    }

    pub(super) fn write_strong(&mut self, s: &crate::mdast::Strong) {
        self.output.push_str("**");
        self.write_inlines(&s.children, Some('*'));
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
        self.write_inlines(&l.children, Some(']'));
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

/// The first character `node` will contribute to the output, if known.
///
/// Only the first character matters to the caller, so nodes that render
/// nothing (empty text) yield `None` and the caller looks further.
fn first_char(node: &crate::mdast::Node) -> Option<char> {
    use crate::mdast::Node;
    match node {
        Node::Text(t) => t.value.chars().next(),
        Node::Html(h) => h.value.chars().next(),
        Node::InlineCode(_) => Some('`'),
        Node::Emphasis(_) | Node::Strong(_) => Some('*'),
        Node::Link(_) => Some('['),
        Node::Image(_) => Some('!'),
        Node::InlineMath(_) => Some('$'),
        Node::Break => Some(' '),
        _ => None,
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
