//! mdast to Quarto Markdown writer
//!
//! Converts an mdast tree into a Quarto Markdown string.

use crate::mdast::{Node, Root};
use serde::Serialize;

mod arguments;
mod block;
mod inline;
pub(crate) mod table_cell;
#[cfg(test)]
mod tests;

/// Physical rendering chosen for the Arguments section.
///
/// This is a presentation decision, so it belongs to the writer rather than
/// to the Rd -> mdast conversion, which emits a semantic
/// [`crate::mdast::Arguments`] node.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArgumentsFormat {
    /// Pipe table - limited to inline content
    PipeTable,
    /// Pandoc grid table - supports block elements in cells
    GridTable,
    /// Quarto list-table (default) - requires Quarto 1.9+
    #[default]
    ListTable,
    /// Markdown loose list - compatible everywhere
    List,
}

/// Options for the QMD writer
#[derive(Debug, Clone, Default)]
pub struct WriterOptions {
    /// Add YAML frontmatter
    pub frontmatter: Option<Frontmatter>,
    /// Use {r} instead of r for R code blocks
    pub quarto_code_blocks: bool,
    /// Physical rendering of the Arguments section
    pub arguments_format: ArgumentsFormat,
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
    writer.write_root(root)
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
}

impl<'a> Writer<'a> {
    fn new(options: &'a WriterOptions) -> Self {
        Self {
            options,
            output: String::new(),
            at_line_start: true,
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
        self.output.push_str("---\n");
        if let Some(title) = &fm.title {
            self.output
                .push_str(&format!(r#"title: "{}""#, escape_yaml_string(title)));
            self.output.push('\n');
        }
        if let Some(pagetitle) = &fm.pagetitle {
            self.output.push_str(&format!(
                r#"pagetitle: "{}""#,
                escape_yaml_string(pagetitle)
            ));
            self.output.push('\n');
        }
        if let Some(format) = &fm.format {
            self.output.push_str(&format!("format: {}\n", format));
        }
        // Write Rd metadata fields
        // TODO: Add configuration to control which metadata fields are output
        if let Some(metadata) = &fm.metadata {
            if let Some(lifecycle) = &metadata.lifecycle {
                self.output.push_str(&format!("lifecycle: {}\n", lifecycle));
            }
            if !metadata.aliases.is_empty() {
                self.output.push_str("aliases:\n");
                for alias in &metadata.aliases {
                    self.output
                        .push_str(&format!(r#"  - "{}""#, escape_yaml_string(alias)));
                    self.output.push('\n');
                }
            }
            if !metadata.keywords.is_empty() {
                self.output.push_str("keywords:\n");
                for keyword in &metadata.keywords {
                    self.output
                        .push_str(&format!(r#"  - "{}""#, escape_yaml_string(keyword)));
                    self.output.push('\n');
                }
            }
            if !metadata.concepts.is_empty() {
                self.output.push_str("concepts:\n");
                for concept in &metadata.concepts {
                    self.output
                        .push_str(&format!(r#"  - "{}""#, escape_yaml_string(concept)));
                    self.output.push('\n');
                }
            }
            if !metadata.source_files.is_empty() {
                self.output.push_str("source-files:\n");
                for source_file in &metadata.source_files {
                    self.output
                        .push_str(&format!(r#"  - "{}""#, escape_yaml_string(source_file)));
                    self.output.push('\n');
                }
            }
        }
        self.output.push_str("---\n\n");
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
            Node::Arguments(a) => self.write_arguments(a),
            Node::DefinitionTerm(_) => {} // Handled by write_definition_list
            Node::DefinitionDescription(_) => {} // Handled by write_definition_list
            Node::Text(t) => self.output.push_str(&t.value),
            Node::Emphasis(e) => self.write_emphasis(e),
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

    /// Emit an already-rendered Markdown block verbatim.
    ///
    /// Used by the Arguments renderers, whose grid-table / list-table / list
    /// shapes are built as raw strings rather than as mdast subtrees.
    fn write_raw_block(&mut self, value: &str) {
        self.output.push_str(value);
        if !value.is_empty() {
            self.at_line_start = value.ends_with('\n');
        }
    }

    fn ensure_newline(&mut self) {
        if !self.at_line_start && !self.output.is_empty() {
            self.output.push('\n');
            self.at_line_start = true;
        }
    }

    fn ensure_blank_line(&mut self) {
        self.ensure_newline();
        if !self.output.ends_with("\n\n") && !self.output.is_empty() {
            self.output.push('\n');
        }
    }
}

fn escape_yaml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
