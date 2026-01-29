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
//!
//! # Features
//!
//! - `json`: Enable JSON serialization/deserialization for `RdDocument`
//! - `lifecycle`: Enable lifecycle badge extraction from Rd documents
//! - `roxygen`: Enable extraction of roxygen2 metadata (source file paths)

pub mod ast;
pub mod lexer;
pub mod parser;

// Feature-gated modules that extend RdDocument with additional methods
#[cfg(feature = "lifecycle")]
mod lifecycle;
#[cfg(feature = "roxygen")]
mod roxygen;

// Re-export main types for convenient access
pub use ast::{DescribeItem, FigureOptions, RdDocument, RdNode, RdSection, SectionTag, SpecialChar};
pub use lexer::{Lexer, Span, Token, TokenKind};
pub use parser::{ParseError, ParseResult, Parser, parse};

// Re-export lifecycle types when the feature is enabled
#[cfg(feature = "lifecycle")]
pub use lifecycle::{Lifecycle, ParseLifecycleError};

// Re-export roxygen types when the feature is enabled
#[cfg(feature = "roxygen")]
pub use roxygen::{RoxygenMetadata, parse_roxygen_comments};
