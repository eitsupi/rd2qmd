//! mdast to Typst writer.
//!
//! Renders the same mdast tree as the Markdown writer into Typst markup
//! (`.typ`), which compiles to PDF and -- with Typst 0.15's experimental
//! `--features html --format html` target -- to HTML.
//!
//! Three conventions are worth stating up front:
//!
//! * **Math.** Rd `\eqn{}`/`\deqn{}` bodies are LaTeX, and Typst math is not
//!   LaTeX, so equations are emitted as [MiTeX] calls (`#mi` inline,
//!   `#mitex` block) and the import is added only to documents that contain
//!   math.
//! * **Code.** R code is written as a plain ```` ```r ```` raw block, never
//!   an executable one. Under plain `typst` it renders as a highlighted
//!   listing; under [calepin] the same block can be executed.
//! * **Raw HTML.** `\out{}` content is emitted through `html.elem`, guarded
//!   by `target()` so PDF compilation of the same file still works.
//!
//! [MiTeX]: https://typst.app/universe/package/mitex/
//! [calepin]: https://vincentarelbundock.github.io/calepin/

use crate::mdast::{ArgumentItem, Node, Root};
use crate::writer::{ArgumentsFormat, Frontmatter};

mod block;
mod html;
mod inline;
#[cfg(test)]
mod tests;

/// Version of the MiTeX package imported by documents containing math.
pub const MITEX_VERSION: &str = "0.2.7";

/// Options for the Typst writer.
#[derive(Debug, Clone, Default)]
pub struct TypstWriterOptions {
    /// Document metadata. Rendered as `#set document(..)` plus a queryable
    /// `#metadata(..)<rd2qmd>` block, and as a visible title heading.
    pub frontmatter: Option<Frontmatter>,
    /// Physical rendering of the Arguments section.
    pub arguments_format: ArgumentsFormat,
}

/// Convert mdast to Typst markup.
pub fn mdast_to_typst(root: &Root, options: &TypstWriterOptions) -> String {
    let mut writer = TypstWriter::new(options);
    writer.write_root(root)
}

/// Typst writer state.
pub(crate) struct TypstWriter<'a> {
    pub(crate) options: &'a TypstWriterOptions,
    pub(crate) output: String,
    pub(crate) at_line_start: bool,
}

impl<'a> TypstWriter<'a> {
    fn new(options: &'a TypstWriterOptions) -> Self {
        Self {
            options,
            output: String::new(),
            at_line_start: true,
        }
    }

    fn write_root(&mut self, root: &Root) -> String {
        if contains_math(&root.children) {
            self.output.push_str(&format!(
                "#import \"@preview/mitex:{MITEX_VERSION}\": mi, mitex\n"
            ));
        }
        if let Some(frontmatter) = &self.options.frontmatter {
            self.write_frontmatter(frontmatter);
        }

        for node in &root.children {
            self.ensure_blank_line();
            self.write_node(node);
        }

        // Exactly one trailing newline.
        while self.output.ends_with("\n\n") {
            self.output.pop();
        }
        if !self.output.ends_with('\n') && !self.output.is_empty() {
            self.output.push('\n');
        }
        self.output.clone()
    }

    /// Emit document metadata: `#set document` for the compiler and PDF/HTML
    /// metadata, a queryable `#metadata(..)<rd2qmd>` for tooling, and a
    /// visible title heading (Typst, unlike Quarto, renders nothing from
    /// document metadata on its own).
    fn write_frontmatter(&mut self, frontmatter: &Frontmatter) {
        if let Some(title) = &frontmatter.title {
            self.output
                .push_str(&format!("#set document(title: {})\n", typst_string(title)));
        }

        let mut fields: Vec<(&str, String)> = Vec::new();
        if let Some(pagetitle) = &frontmatter.pagetitle {
            fields.push(("pagetitle", typst_string(pagetitle)));
        }
        if let Some(metadata) = &frontmatter.metadata {
            if let Some(lifecycle) = &metadata.lifecycle {
                fields.push(("lifecycle", typst_string(lifecycle)));
            }
            for (key, values) in [
                ("aliases", &metadata.aliases),
                ("keywords", &metadata.keywords),
                ("concepts", &metadata.concepts),
                ("source-files", &metadata.source_files),
            ] {
                if !values.is_empty() {
                    fields.push((key, typst_string_array(values)));
                }
            }
        }
        if !fields.is_empty() {
            self.output.push_str("#metadata((\n");
            for (key, value) in fields {
                // `source-files` is not a bare Typst identifier.
                let key = if key.contains('-') {
                    typst_string(key)
                } else {
                    key.to_owned()
                };
                self.output.push_str(&format!("  {key}: {value},\n"));
            }
            self.output.push_str("))<rd2qmd>\n");
        }

        if let Some(title) = &frontmatter.title {
            self.output.push('\n');
            self.output.push_str("= ");
            self.output.push_str(&escape_text(title));
            self.output.push('\n');
        }
        self.at_line_start = true;
    }

    pub(crate) fn write_node(&mut self, node: &Node) {
        match node {
            // Block nodes
            Node::Heading(h) => self.write_heading(h),
            Node::Paragraph(p) => self.write_paragraph(p),
            Node::ThematicBreak => self.write_thematic_break(),
            Node::Blockquote(b) => self.write_blockquote(b),
            Node::List(l) => self.write_list(l, 0),
            Node::ListItem(_) => {} // Handled by write_list
            Node::Code(c) => self.write_code(c),
            Node::Table(t) => self.write_table(t),
            Node::TableRow(_) => {}  // Handled by write_table
            Node::TableCell(_) => {} // Handled by write_table
            Node::DefinitionList(dl) => self.write_definition_list(dl),
            Node::DefinitionTerm(_) => {} // Handled by write_definition_list
            Node::DefinitionDescription(_) => {} // Handled by write_definition_list
            Node::Arguments(a) => self.write_arguments(a),

            // Inline nodes
            Node::Text(t) => {
                let escaped = escape_text_at(&t.value, self.at_line_start);
                self.output.push_str(&escaped);
                // Whitespace does not end the start-of-line state: Typst reads
                // an indented `-` as a list marker too, and inline text often
                // arrives split across several Text nodes.
                if !escaped.trim().is_empty() || escaped.contains('\n') {
                    self.at_line_start = escaped.ends_with('\n');
                }
            }
            Node::Emphasis(e) => self.write_emphasis(e),
            Node::Strong(s) => self.write_strong(s),
            Node::InlineCode(c) => self.write_inline_code(c),
            Node::Break => self.write_break(),
            Node::Link(l) => self.write_link(l),
            Node::Image(i) => self.write_image(i),
            Node::Math(m) => self.write_math(m),
            Node::InlineMath(m) => self.write_inline_math(m),
            Node::Html(h) => self.write_html(h),
        }
    }

    /// Write inline children into a `[..]` content block.
    pub(crate) fn write_content_block(&mut self, children: &[Node]) {
        self.output.push('[');
        self.at_line_start = false;
        for child in children {
            self.write_node(child);
        }
        self.output.push(']');
        self.at_line_start = false;
    }

    /// Render nodes to a standalone Typst string, isolated from the current
    /// output buffer's line state.
    ///
    /// Blank lines separate block nodes only: inline siblings (a table cell
    /// is a flat run of them) must stay on one line.
    pub(crate) fn render_isolated(&self, nodes: &[Node]) -> String {
        let mut writer = TypstWriter::new(self.options);
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 && (is_block(node) || is_block(&nodes[i - 1])) {
                writer.ensure_blank_line();
            }
            writer.write_node(node);
        }
        writer.output.trim().to_owned()
    }

    pub(crate) fn ensure_newline(&mut self) {
        if !self.at_line_start && !self.output.is_empty() {
            self.output.push('\n');
            self.at_line_start = true;
        }
    }

    pub(crate) fn ensure_blank_line(&mut self) {
        self.ensure_newline();
        if !self.output.ends_with("\n\n") && !self.output.is_empty() {
            self.output.push('\n');
        }
    }
}

/// Whether a node is block-level, and so needs its own line.
fn is_block(node: &Node) -> bool {
    matches!(
        node,
        Node::Heading(_)
            | Node::Paragraph(_)
            | Node::ThematicBreak
            | Node::Blockquote(_)
            | Node::List(_)
            | Node::ListItem(_)
            | Node::Code(_)
            | Node::Table(_)
            | Node::DefinitionList(_)
            | Node::Arguments(_)
            | Node::Math(_)
    )
}

/// Whether any node in the tree is math, i.e. whether MiTeX must be imported.
fn contains_math(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Math(_) | Node::InlineMath(_) => true,
        Node::Heading(n) => contains_math(&n.children),
        Node::Paragraph(n) => contains_math(&n.children),
        Node::Blockquote(n) => contains_math(&n.children),
        Node::List(n) => contains_math(&n.children),
        Node::ListItem(n) => contains_math(&n.children),
        Node::Table(n) => contains_math(&n.children),
        Node::TableRow(n) => contains_math(&n.children),
        Node::TableCell(n) => contains_math(&n.children),
        Node::DefinitionList(n) => contains_math(&n.children),
        Node::DefinitionTerm(n) => contains_math(&n.children),
        Node::DefinitionDescription(n) => contains_math(&n.children),
        Node::Emphasis(n) => contains_math(&n.children),
        Node::Strong(n) => contains_math(&n.children),
        Node::Link(n) => contains_math(&n.children),
        Node::Arguments(n) => n
            .items
            .iter()
            .any(|item: &ArgumentItem| contains_math(&item.description)),
        _ => false,
    })
}

/// Escape a Typst string literal, including the surrounding quotes.
pub(crate) fn typst_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}

fn typst_string_array(values: &[String]) -> String {
    let items: Vec<_> = values.iter().map(|value| typst_string(value)).collect();
    // A one-element Typst array needs a trailing comma to stay an array.
    if items.len() == 1 {
        format!("({},)", items[0])
    } else {
        format!("({})", items.join(", "))
    }
}

/// Escape text for Typst markup, assuming it is not at the start of a line.
pub(crate) fn escape_text(value: &str) -> String {
    escape_text_at(value, false)
}

/// Escape text for Typst markup.
///
/// Two classes of character need escaping. The first is special anywhere:
/// `\` (escape), `#` (code), `$` (math), `*`/`_` (strong/emph), `` ` ``
/// (raw), `<`/`>` (labels), `@` (references), `~` (non-breaking space) and
/// `[`/`]` (content blocks), and parentheses (which can call the preceding
/// content expression). The second is special only at the start of a
/// line, where it would begin a heading, list, or term: `=`, `-`, `+`, `/`
/// and a digit run followed by `.`.
pub(crate) fn escape_text_at(value: &str, at_line_start: bool) -> String {
    let mut out = String::with_capacity(value.len());
    let mut line_start = at_line_start;

    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if line_start {
            // Leading whitespace does not end the "start of line" state:
            // Typst treats an indented `-` as a list marker too.
            if character == ' ' || character == '\t' {
                out.push(character);
                continue;
            }
            match character {
                '=' | '-' | '+' | '/' => {
                    out.push('\\');
                    out.push(character);
                    line_start = false;
                    continue;
                }
                '0'..='9' => {
                    // A digit run followed by `.` starts an enumeration.
                    let mut digits = String::from(character);
                    while let Some(next) = chars.peek().copied() {
                        if next.is_ascii_digit() {
                            digits.push(next);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    out.push_str(&digits);
                    if chars.peek() == Some(&'.') {
                        chars.next();
                        out.push_str("\\.");
                    }
                    line_start = false;
                    continue;
                }
                _ => {}
            }
        }

        match character {
            // A period immediately following a `#function(..)` expression is
            // parsed as field access. Escaping every prose period is harmless
            // and keeps Text nodes safe regardless of their previous sibling.
            '\\' | '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '~' | '[' | ']' | '(' | ')'
            | '.' => {
                out.push('\\');
                out.push(character);
                line_start = false;
            }
            // Typst turns `--` and `---` into en/em dashes. Rd prose often
            // contains CLI flags and literal numeric ranges, so preserve runs.
            '-' if chars.peek() == Some(&'-') => {
                out.push('\\');
                out.push(character);
                while chars.peek() == Some(&'-') {
                    out.push('\\');
                    out.push(chars.next().expect("peeked hyphen must exist"));
                }
                line_start = false;
            }
            '\n' => {
                out.push('\n');
                line_start = true;
            }
            _ => {
                out.push(character);
                line_start = false;
            }
        }
    }

    out
}

/// Indent every line after the first by `indent` spaces.
pub(crate) fn indent_continuation(value: &str, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::with_capacity(value.len());
    for (i, line) in value.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(&pad);
            }
        }
        out.push_str(line);
    }
    out
}
