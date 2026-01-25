//! rd2qmd-core: Core library for converting Rd files to Quarto Markdown
//!
//! This crate provides:
//! - Rd file parsing (lexer + recursive descent parser)
//! - Rd AST representation
//! - Rd AST to mdast conversion
//! - mdast to Quarto Markdown output

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::{RdDocument, RdNode, RdSection, SectionTag};
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::{ParseError, Parser, parse};
