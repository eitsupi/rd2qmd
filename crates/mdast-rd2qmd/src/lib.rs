//! mdast-rd2qmd: mdast types and Quarto Markdown writer for rd2qmd
//!
//! This crate provides:
//! - mdast (Markdown Abstract Syntax Tree) types (subset)
//! - Serialization to Quarto Markdown format
//!
//! ## Example
//!
//! ```rust
//! use mdast_rd2qmd::{Node, Root, mdast_to_qmd, WriterOptions};
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
pub use writer::{mdast_to_qmd, Frontmatter, WriterOptions};
