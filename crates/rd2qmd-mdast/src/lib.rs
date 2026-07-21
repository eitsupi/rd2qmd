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
pub use writer::{
    Frontmatter, RdMetadata, WriterOptions, escape_link_title, format_link_destination,
    mdast_to_qmd,
};

/// Format an inline code value as a Markdown code span, with safe backtick fencing.
///
/// Chooses a fence one backtick longer than the longest consecutive backtick run
/// in `value`, so the delimiter never appears inside the span. Adds padding spaces
/// when a multi-backtick fence is used, to prevent ambiguity when the code starts or
/// ends with a backtick character. Values beginning with `r` plus ASCII whitespace
/// also use a padded double-backtick fence so Quarto's knitr engine cannot mistake
/// literal code for an executable inline R expression.
///
/// Also prepends a space when `prev_ends_with_backtick` is true to prevent
/// adjacent backtick spans from merging into a single code span.
pub fn format_inline_code(value: &str, prev_ends_with_backtick: bool) -> String {
    let mut out = String::new();
    if prev_ends_with_backtick {
        out.push(' ');
    }

    let max_run = value
        .chars()
        .fold((0usize, 0usize), |(max, cur), c| {
            if c == '`' {
                (max.max(cur + 1), cur + 1)
            } else {
                (max, 0)
            }
        })
        .0;

    let looks_like_inline_r = value
        .strip_prefix('r')
        .and_then(|rest| rest.chars().next())
        .is_some_and(|character| character.is_ascii_whitespace());

    if max_run == 0 && !looks_like_inline_r {
        out.push('`');
        out.push_str(value);
        out.push('`');
    } else {
        let fence = "`".repeat(if max_run == 0 { 2 } else { max_run + 1 });
        out.push_str(&fence);
        out.push(' ');
        out.push_str(value);
        out.push(' ');
        out.push_str(&fence);
    }

    out
}
