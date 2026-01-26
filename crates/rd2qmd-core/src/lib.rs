//! rd2qmd-core: Core library for converting Rd files to Quarto Markdown
//!
//! This crate provides:
//! - Rd file parsing (via rd-parser crate)
//! - Rd AST to mdast conversion
//! - mdast to Quarto Markdown output (via mdast-rd2qmd crate)

pub mod convert;

// Re-export rd-parser types
pub use rd_parser::{RdDocument, RdNode, RdSection, SectionTag, parse};

// Re-export mdast-rd2qmd types
pub use mdast_rd2qmd::{Frontmatter, WriterOptions, mdast_to_qmd};

pub use convert::{ConverterOptions, rd_to_mdast, rd_to_mdast_with_options};
