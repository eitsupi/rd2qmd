//! Converts a canonical `rd_ast::RdDocument` to a Quarto-flavored mdast tree.

mod assembly;
mod blocks;
mod code;
mod document;
mod inline;
mod leaf_text;
mod roxygen;
mod traversal;

/// Whether a node is the validated RDS help-database marker for a user macro
/// definition. The definition's children are producer metadata; its expansion
/// is represented by the following sibling and is rendered separately.
pub(crate) fn is_usermacro_definition(node: &rd_ast::RdNode) -> bool {
    node.as_raw().is_some_and(|raw| {
        matches!(
            rd_ast::classify_raw_node(raw),
            rd_ast::RawNodeClassification::ExpectedUserMacroDefinition
        )
    })
}

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
#[allow(unused_imports)]
pub(crate) use roxygen::{RoxygenCodeBlock, try_match_roxygen_code_block};
#[allow(unused_imports)]
pub(crate) use traversal::{BlockContentItem, ParagraphItem, scan_block_content};
