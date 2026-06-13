//! rd2qmd-mdast: mdast types and Quarto Markdown writer for rd2qmd
//!
//! This crate provides:
//! - mdast (Markdown Abstract Syntax Tree) types (subset)
//! - Serialization to Quarto Markdown format
//!
//! ## Example
//!
//! ```rust
//! use rd2qmd_mdast::{Node, Root, mdast_to_qmd, WriterOptions};
//!
//! let doc = Root::new(vec![
//!     Node::heading(1, vec![Node::text("Hello")]),
//!     Node::paragraph(vec![Node::text("World")]),
//! ]);
//!
//! let qmd = mdast_to_qmd(&doc, &WriterOptions::default());
//! assert!(qmd.contains("# Hello"));
//! ```

pub mod mdast;
pub mod writer;

pub use mdast::{
    Align, Blockquote, Code, DefinitionDescription, DefinitionList, DefinitionTerm, Emphasis,
    Heading, Html, Image, InlineCode, InlineMath, Link, List, ListItem, Math, Node, Paragraph,
    Root, Strong, Table, TableCell, TableRow, Text,
};
pub use writer::{Frontmatter, RdMetadata, WriterOptions, mdast_to_qmd};

/// Format an inline code value as a Markdown code span, with safe backtick fencing.
///
/// If `value` contains backticks, uses double-backtick delimiters with padding
/// (`` `` `value` `` ``); otherwise uses single backticks (`` `value` ``).
///
/// Also prepends a space when `prev_ends_with_backtick` is true to prevent
/// adjacent backtick spans from merging into a single code span.
pub fn format_inline_code(value: &str, prev_ends_with_backtick: bool) -> String {
    let mut out = String::new();
    if prev_ends_with_backtick {
        out.push(' ');
    }
    if value.contains('`') {
        out.push_str("`` ");
        out.push_str(value);
        out.push_str(" ``");
    } else {
        out.push('`');
        out.push_str(value);
        out.push('`');
    }
    out
}
