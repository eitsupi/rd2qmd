//! rd-parser: Parser for R Documentation (Rd) files
//!
//! This crate provides:
//! - Rd file lexer (tokenizer)
//! - Recursive descent parser
//! - Rd AST types
//!
//! # Example
//!
//! ```
//! use rd_parser::{parse, RdDocument, SectionTag};
//!
//! let source = r#"
//! \name{example}
//! \title{Example Function}
//! \description{An example function.}
//! "#;
//!
//! let doc = parse(source).unwrap();
//! assert_eq!(doc.sections.len(), 3);
//! ```

pub mod ast;
pub mod lexer;
pub mod parser;

// Re-export main types for convenient access
pub use ast::{DescribeItem, RdDocument, RdNode, RdSection, SectionTag, SpecialChar};
pub use lexer::{Lexer, Span, Token, TokenKind};
pub use parser::{parse, ParseError, ParseResult, Parser};
