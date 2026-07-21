//! Converts a canonical `rd_ast::RdDocument` to a Quarto-flavored mdast tree.

mod assembly;
mod blocks;
mod code;
mod document;
mod inline;
mod leaf_text;
#[cfg(feature = "roxygen")]
mod roxygen;
mod traversal;

pub(crate) use assembly::convert_document;
#[allow(unused_imports)]
pub(crate) use blocks::{
    BlockConversionContext, convert_arguments, convert_block_content, convert_custom_section,
};
#[allow(unused_imports)]
pub(crate) use code::{ExampleOptions, convert_examples, convert_usage};
#[allow(unused_imports)]
pub(crate) use document::{
    CustomSection, DocumentMetadata, DocumentSection, DocumentStructure, FixedSection,
    FixedSectionBody, FixedSectionKind, build_custom_sections, build_document_structure,
    extract_document_metadata,
};
#[allow(unused_imports)]
pub(crate) use inline::{
    InlineConversionContext, LinkResolutionContext, convert_inline_node, convert_inline_nodes,
    extract_plain_text,
};
#[cfg(feature = "roxygen")]
#[allow(unused_imports)]
pub(crate) use roxygen::{RoxygenCodeBlock, try_match_roxygen_code_block};
#[allow(unused_imports)]
pub(crate) use traversal::{BlockContentItem, ParagraphItem, scan_block_content};
