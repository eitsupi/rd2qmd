//! rd2qmd-core: Core library for converting Rd files to Quarto Markdown
//!
//! This crate provides:
//! - Rd file parsing (lexer + recursive descent parser)
//! - Rd AST representation
//! - Rd AST to mdast conversion
//! - mdast to Quarto Markdown output

pub mod ast;
pub mod convert;
pub mod lexer;
pub mod mdast;
pub mod parser;
pub mod writer;

pub use ast::{RdDocument, RdNode, RdSection, SectionTag};
pub use convert::rd_to_mdast;
pub use lexer::{Lexer, Token, TokenKind};
pub use mdast::{Node as MdNode, Root as MdRoot};
pub use parser::{ParseError, Parser, parse};
pub use writer::{WriterOptions, mdast_to_qmd};
