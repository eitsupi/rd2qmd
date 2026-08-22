//! General block-content assembly for the rd_ast conversion migration.

use rd_ast::{RdArgument, RdNode};
use rd2qmd_mdast::{ArgumentItem, Node};

use super::{
    inline::{self, InlineConversionContext},
    leaf_text::flatten_verbatim_leaves,
    traversal::{BlockContentItem, scan_block_content},
};
use tag_conversion::{convert_block, convert_paragraph};

mod tag_conversion;
#[cfg(test)]
mod tests;

/// Borrowed configuration used while converting general block content.
#[derive(Clone, Copy)]
pub(crate) struct BlockConversionContext<'a> {
    pub(crate) inline: InlineConversionContext<'a>,
    pub(crate) prefer_ascii_math: bool,
    pub(crate) enclosing_heading_depth: u8,
}

/// Convert paragraphs and supported semantic blocks in source order.
pub(crate) fn convert_block_content(
    nodes: &[RdNode],
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    scan_block_content(nodes)
        .into_iter()
        .flat_map(|item| match item {
            BlockContentItem::Paragraph(items) => {
                convert_paragraph(items, context).into_iter().collect()
            }
            BlockContentItem::Block(node) => convert_block(node, context),
            BlockContentItem::RoxygenCode(block) => vec![Node::code(block.language, block.code)],
        })
        .collect()
}

/// Convert one custom section tree while preserving nested source positions.
pub(crate) fn convert_custom_section(
    section: &super::document::CustomSection<'_>,
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    let depth = (2usize + section.nesting).min(6) as u8;
    let mut nodes = vec![Node::heading(
        depth,
        inline::convert_inline_nodes(section.title, &context.inline),
    )];
    nodes.extend(convert_custom_section_body(section, context, depth));
    nodes
}

fn convert_custom_section_body(
    section: &super::document::CustomSection<'_>,
    context: &BlockConversionContext<'_>,
    depth: u8,
) -> Vec<Node> {
    let child_context = BlockConversionContext {
        enclosing_heading_depth: depth,
        ..*context
    };
    let mut nodes = Vec::new();
    let mut cursor = 0;
    for child in &section.children {
        nodes.extend(convert_block_content(
            &section.body[cursor..child.source_index],
            &child_context,
        ));
        nodes.extend(convert_custom_section(child, &child_context));
        cursor = child.source_index + 1;
    }
    nodes.extend(convert_block_content(
        &section.body[cursor..],
        &child_context,
    ));
    nodes
}

/// Convert an already-structured Arguments section into a semantic
/// [`Node::Arguments`].
///
/// The physical shape (pipe/grid/list table, loose list, a Typst `#table`)
/// is a presentation choice and belongs to the writer, so nothing here is
/// pre-rendered: each entry keeps its name as plain text and its description
/// as block-level nodes.
pub(crate) fn convert_arguments(
    arguments: &[RdArgument<'_>],
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    if arguments.is_empty() {
        return Vec::new();
    }

    let items = arguments
        .iter()
        .map(|argument| ArgumentItem {
            name: argument_name(argument, context),
            description: convert_block_content(argument.description, context),
        })
        .collect();

    vec![Node::arguments(items)]
}

fn argument_name(argument: &RdArgument<'_>, context: &BlockConversionContext<'_>) -> String {
    inline::extract_plain_text(&inline::convert_inline_nodes(
        argument.name,
        &context.inline,
    ))
}

pub(super) fn recover_verbatim(nodes: &[RdNode]) -> String {
    flatten_verbatim_leaves(nodes).unwrap_or_else(|error| error.recovered_text().to_owned())
}

/// Replace line endings with a single space, leaving every other byte of
/// whitespace (including same-line runs of spaces/tabs) untouched. Used
/// wherever text must become single-line-safe (e.g. for a pipe-table row)
/// without corrupting other meaningful whitespace, such as intentional code-alignment spacing or ASCII-equation layout.
pub(super) fn replace_line_endings_with_space(value: &str) -> String {
    value.replace("\r\n", " ").replace(['\r', '\n'], " ")
}
