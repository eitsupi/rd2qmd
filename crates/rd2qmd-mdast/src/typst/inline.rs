//! Inline-level Typst writers.

use super::{TypstWriter, typst_string};
use crate::mdast::Node;

impl TypstWriter<'_> {
    pub(super) fn write_emphasis(&mut self, e: &crate::mdast::Emphasis) {
        self.output.push_str("#emph");
        self.write_content_block(&e.children);
    }

    pub(super) fn write_strong(&mut self, s: &crate::mdast::Strong) {
        self.output.push_str("#strong");
        self.write_content_block(&s.children);
    }

    pub(super) fn write_inline_code(&mut self, c: &crate::mdast::InlineCode) {
        self.output.push_str(&raw_span(&c.value));
        self.at_line_start = false;
    }

    pub(super) fn write_break(&mut self) {
        // Typst's forced line break.
        self.output.push_str(" \\\n");
        self.at_line_start = true;
    }

    pub(super) fn write_link(&mut self, l: &crate::mdast::Link) {
        self.output.push_str("#link(");
        self.output.push_str(&typst_string(&l.url));
        self.output.push(')');
        // `#link("url")` alone renders the URL itself, so the content block is
        // only needed when the label differs from the destination.
        let label_is_url = matches!(l.children.as_slice(), [Node::Text(t)] if t.value == l.url);
        if !label_is_url {
            self.write_content_block(&l.children);
        }
        self.at_line_start = false;
    }

    pub(super) fn write_image(&mut self, img: &crate::mdast::Image) {
        self.output.push_str("#image(");
        self.output.push_str(&typst_string(&img.url));
        if !img.alt.is_empty() {
            self.output.push_str(", alt: ");
            self.output.push_str(&typst_string(&img.alt));
        }
        self.output.push(')');
        self.at_line_start = false;
    }

    /// Rd equations are LaTeX, which Typst's own math mode cannot read, so
    /// they go through MiTeX.
    pub(super) fn write_math(&mut self, m: &crate::mdast::Math) {
        self.ensure_newline();
        self.output.push_str("#mitex(");
        self.output.push_str(&latex_argument(&m.value));
        self.output.push_str(")\n");
        self.at_line_start = true;
    }

    pub(super) fn write_inline_math(&mut self, m: &crate::mdast::InlineMath) {
        self.output.push_str("#mi(");
        self.output.push_str(&latex_argument(&m.value));
        self.output.push(')');
        self.at_line_start = false;
    }
}

/// Render a LaTeX fragment as a MiTeX argument.
///
/// A raw block is preferred, since LaTeX is mostly backslashes and a raw
/// block needs no escaping at all; a fragment containing a backtick falls
/// back to a string literal.
fn latex_argument(value: &str) -> String {
    let value = value.trim();
    if value.contains('`') {
        typst_string(value)
    } else {
        format!("`{value}`")
    }
}

/// Render a value as inline Typst raw text.
///
/// Typst raw spans are delimited by a single backtick (or three, which also
/// consume a leading language tag), so a value containing a backtick cannot
/// be expressed as a span and uses `#raw` instead.
pub(super) fn raw_span(value: &str) -> String {
    if value.contains('`') {
        return format!("#raw({})", typst_string(value));
    }
    // A leading or trailing space would be trimmed by the raw span.
    if value.starts_with(' ') || value.ends_with(' ') {
        return format!("#raw({})", typst_string(value));
    }
    format!("`{value}`")
}

/// Render a value as a Typst raw block with an optional language tag.
///
/// R code is emitted as a plain ```` ```r ```` block: Typst renders it as a
/// highlighted listing, and calepin can execute the same block.
pub(super) fn raw_block(value: &str, lang: Option<&str>) -> String {
    let value = value.strip_suffix('\n').unwrap_or(value);
    // Typst raw blocks are always three backticks, with no longer-fence
    // escape hatch: a run of three inside the content closes the block early
    // (and even where it currently parses, Typst warns that a future version
    // will read what follows as a language tag). Fall back to `#raw`.
    if value.contains("```") {
        let lang_argument = lang
            .map(|lang| format!("lang: {}, ", typst_string(lang)))
            .unwrap_or_default();
        return format!("#raw(block: true, {lang_argument}{})", typst_string(value));
    }
    let mut out = String::from("```");
    if let Some(lang) = lang {
        out.push_str(lang);
    }
    out.push('\n');
    out.push_str(value);
    out.push('\n');
    out.push_str("```");
    out
}
