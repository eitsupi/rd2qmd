//! rd2qmd-core: Core library for converting Rd files to Quarto Markdown
//!
//! This crate provides:
//! - Rd file parsing (via rd-parser crate)
//! - Rd AST to mdast conversion
//! - mdast to Quarto Markdown output

pub mod convert;
pub mod mdast;
pub mod writer;

// Re-export rd-parser types for backward compatibility
pub use rd_parser::{
    Lexer, ParseError, Parser, RdDocument, RdNode, RdSection, SectionTag, Token, TokenKind, parse,
};

pub use convert::{ConverterOptions, rd_to_mdast, rd_to_mdast_with_options};
pub use mdast::{Node as MdNode, Root as MdRoot};
pub use writer::{WriterOptions, mdast_to_qmd};
