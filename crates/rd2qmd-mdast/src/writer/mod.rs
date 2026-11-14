//! mdast to Quarto Markdown writer
//!
//! Converts an mdast tree into a Quarto Markdown string.

use crate::mdast::{Node, Root};
use serde::Serialize;

mod block;
mod inline;
#[cfg(test)]
mod structure;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_boundary;
#[cfg(test)]
mod tests_dl_list;
#[cfg(test)]
mod tests_emphasis;
#[cfg(test)]
mod tests_fragment;
#[cfg(test)]
mod tests_quarto_dl;
#[cfg(test)]
mod tests_structure;

/// Options for the QMD writer
#[derive(Debug, Clone, Default)]
pub struct WriterOptions {
    /// Add YAML frontmatter
    pub frontmatter: Option<Frontmatter>,
    /// Use {r} instead of r for R code blocks
    pub quarto_code_blocks: bool,
    /// Syntax for [`Node::DefinitionList`]
    pub definition_list_syntax: DefinitionListSyntax,
}

/// Markdown syntax used to write a definition list.
///
/// Both forms carry the same structure; they differ in which renderers
/// understand them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DefinitionListSyntax {
    /// Pandoc's `definition_lists` extension: the term on its own line, then
    /// each description after a `:   ` marker.
    #[default]
    Pandoc,
    /// Quarto's `::: {.definition-list}` div. It holds a bullet list whose
    /// items are a term followed by a bullet list with one item per
    /// description.
    QuartoDiv,
}

/// YAML frontmatter content
#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    pub title: Option<String>,
    /// Page title for browser tab/SEO (pkgdown style: "<title> — <name>")
    pub pagetitle: Option<String>,
    pub format: Option<String>,
    /// Rd metadata (lifecycle, aliases, keywords, concepts)
    pub metadata: Option<RdMetadata>,
}

/// Metadata extracted from Rd documentation
///
/// This struct is shared between topic index generation and frontmatter output.
/// For frontmatter, the writer converts lifecycle to string via `as_str()`.
/// For topic index, serde serializes lifecycle as lowercase string.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RdMetadata {
    /// Lifecycle stage (deprecated, experimental, superseded, stable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<String>,
    /// Aliases for this topic (from \alias{})
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Keywords for this topic (from \keyword{})
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// Concepts for this topic (from \concept{})
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<String>,
    /// Source R files that generated this Rd file (from roxygen2 comments)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub source_files: Vec<String>,
}

/// Convert mdast to Quarto Markdown
pub fn mdast_to_qmd(root: &Root, options: &WriterOptions) -> String {
    let mut writer = Writer::new(options);
    writer.write_root(&Root::new(crate::normalize_inline_boundaries(
        &root.children,
    )))
}

/// Render block nodes as a standalone Markdown fragment.
///
/// Behaves like [`mdast_to_qmd`] with `options.frontmatter` ignored, and trims
/// all leading and trailing whitespace from the result, so the fragment has
/// no trailing newline and can be embedded in a larger construct (for example
/// a table or list cell) that supplies its own indentation and separators.
pub fn render_blocks_fragment(nodes: &[Node], options: &WriterOptions) -> String {
    let options = WriterOptions {
        frontmatter: None,
        ..options.clone()
    };
    mdast_to_qmd(&Root::new(nodes.to_vec()), &options)
        .trim()
        .to_owned()
}

/// Format the identified CommonMark-unsafe URL cases as a link destination.
///
/// Line endings are flattened to spaces. Destinations containing whitespace,
/// ASCII control characters, angle brackets, or parentheses use the bracketed
/// destination fallback. This is a deliberate scope boundary, not a claim of
/// exhaustive CommonMark round-trip safety.
pub fn format_link_destination(url: &str) -> String {
    let url = replace_line_endings_with_space(url);
    if url.chars().any(|c| {
        c.is_ascii_whitespace() || c.is_ascii_control() || matches!(c, '<' | '>' | '(' | ')')
    }) {
        let escaped = url
            .replace('\\', "\\\\")
            .replace('<', "\\<")
            .replace('>', "\\>");
        format!("<{escaped}>")
    } else {
        url
    }
}

/// Escape text for a double-quoted Markdown link title.
pub fn escape_link_title(title: &str) -> String {
    let title = replace_line_endings_with_space(title);
    title.replace('\\', "\\\\").replace('"', "\\\"")
}

fn replace_line_endings_with_space(value: &str) -> String {
    value.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

/// QMD writer state
struct Writer<'a> {
    options: &'a WriterOptions,
    output: String,
    /// Whether we're at the start of a line
    at_line_start: bool,
    /// First character of the inline sibling after the node being written
    next_char: Option<char>,
}

impl<'a> Writer<'a> {
    fn new(options: &'a WriterOptions) -> Self {
        Self {
            options,
            output: String::new(),
            at_line_start: true,
            next_char: None,
        }
    }

    fn write_root(&mut self, root: &Root) -> String {
        // Write frontmatter if provided
        if let Some(fm) = &self.options.frontmatter {
            self.write_frontmatter(fm);
        }

        // Write children
        for (i, node) in root.children.iter().enumerate() {
            if i > 0 {
                self.ensure_blank_line();
            }
            self.write_node(node);
        }

        self.output.clone()
    }

    fn write_frontmatter(&mut self, fm: &Frontmatter) {
        self.output.push_str(&frontmatter_yaml(fm));
        self.output.push('\n');
    }

    fn write_node(&mut self, node: &Node) {
        match node {
            Node::Heading(h) => self.write_heading(h),
            Node::Paragraph(p) => self.write_paragraph(p),
            Node::ThematicBreak => self.write_thematic_break(),
            Node::Blockquote(b) => self.write_blockquote(b),
            Node::List(l) => self.write_list(l),
            Node::ListItem(li) => self.write_list_item(li),
            Node::Code(c) => self.write_code(c),
            Node::Table(t) => self.write_table(t),
            Node::TableRow(_) => {}  // Handled by write_table
            Node::TableCell(_) => {} // Handled by write_table
            Node::DefinitionList(dl) => self.write_definition_list(dl),
            Node::DefinitionTerm(_) => {} // Handled by write_definition_list
            Node::DefinitionDescription(_) => {} // Handled by write_definition_list
            Node::Text(t) => self.output.push_str(&t.value),
            Node::Emphasis(e) => self.write_emphasis(&e.children),
            Node::Strong(s) => self.write_strong(s),
            Node::InlineCode(c) => self.write_inline_code(c),
            Node::Break => self.write_break(),
            Node::Link(l) => self.write_link(l),
            Node::Image(img) => self.write_image(img),
            Node::Math(m) => self.write_math(m),
            Node::InlineMath(m) => self.write_inline_math(m),
            Node::Html(h) => {
                self.output.push_str(&h.value);
                if !h.value.is_empty() {
                    self.at_line_start = h.value.ends_with('\n');
                }
            }
        }
    }

    // Helper methods

    fn ensure_newline(&mut self) {
        if !self.at_line_start && !self.output.is_empty() {
            self.output.push('\n');
            self.at_line_start = true;
        }
    }

    /// End the current line of a block fragment.
    ///
    /// Every non-empty block fragment ends with exactly one newline. Block
    /// writers that own their last line (headings, code fences, ...) write
    /// it themselves; containers call this after writing inline content
    /// that does not, such as a paragraph inside a list item.
    fn write_terminator(&mut self) {
        self.output.push('\n');
        self.at_line_start = true;
    }

    /// Write the blank line that separates two sibling blocks inside a
    /// container. The previous sibling must already be terminated.
    fn write_separator(&mut self) {
        debug_assert!(
            self.output.ends_with('\n'),
            "separator after an unterminated block: {:?}",
            self.output
        );
        self.output.push('\n');
        self.at_line_start = true;
    }

    /// End the current line after a child whose output is not covered by the
    /// terminator contract: raw HTML is written verbatim and may lack a final
    /// newline, and an empty table or list writes nothing at all. A no-op
    /// after any other block.
    fn close_opaque_block(&mut self) {
        if !self.output.ends_with('\n') {
            self.write_terminator();
        }
    }

    fn ensure_blank_line(&mut self) {
        self.ensure_newline();
        if !self.output.ends_with("\n\n") && !self.output.is_empty() {
            self.output.push('\n');
        }
    }
}

/// Render frontmatter as a YAML block between `---` lines.
///
/// The YAML is written by saphyr's emitter, with every string value double
/// quoted and escaped, so it is valid for any input. Fields keep a fixed
/// order and absent fields are left out.
fn frontmatter_yaml(fm: &Frontmatter) -> String {
    use saphyr::{Mapping, Scalar, Yaml, YamlEmitter};
    use std::borrow::Cow;

    fn string(value: &str) -> Yaml<'_> {
        Yaml::Value(Scalar::String(Cow::Borrowed(value)))
    }
    fn strings(values: &[String]) -> Yaml<'_> {
        Yaml::Sequence(values.iter().map(|v| string(v)).collect())
    }

    // Titles are prose that Rd sources wrap across lines; the breaks carry
    // no meaning.
    let title = fm.title.as_deref().map(single_line);
    let pagetitle = fm.pagetitle.as_deref().map(single_line);

    let mut map = Mapping::new();
    let mut insert = |key: &'static str, value| {
        map.insert(string(key), value);
    };
    if let Some(title) = &title {
        insert("title", string(title));
    }
    if let Some(pagetitle) = &pagetitle {
        insert("pagetitle", string(pagetitle));
    }
    if let Some(format) = &fm.format {
        insert("format", string(format));
    }
    // TODO: Add configuration to control which metadata fields are output
    if let Some(metadata) = &fm.metadata {
        if let Some(lifecycle) = &metadata.lifecycle {
            insert("lifecycle", string(lifecycle));
        }
        for (key, values) in [
            ("aliases", &metadata.aliases),
            ("keywords", &metadata.keywords),
            ("concepts", &metadata.concepts),
            ("source-files", &metadata.source_files),
        ] {
            if !values.is_empty() {
                insert(key, strings(values));
            }
        }
    }

    if map.is_empty() {
        return "---\n---\n".to_owned();
    }
    let mut out = String::new();
    let mut emitter = YamlEmitter::new(&mut out);
    emitter.quoted_value(true);
    emitter
        .dump(&Yaml::Mapping(map))
        .expect("emitting YAML into a String cannot fail");
    // The emitter starts the document with `---` and leaves the last line
    // open.
    out.push_str("\n---\n");
    out
}

/// Collapse runs of ASCII whitespace, including line breaks, into single
/// spaces. Other whitespace, such as a no-break space, is kept.
fn single_line(s: &str) -> String {
    s.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}
