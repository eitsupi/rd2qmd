//! General block-content assembly for the rd_ast conversion migration.

use rd_ast::{RdArgument, RdNode};
use rd2qmd_mdast::{Align, Html, Node};
use tabled::settings::Style;
use tabled::settings::style::HorizontalLine;

use super::{
    inline::{self, InlineConversionContext},
    leaf_text::flatten_verbatim_leaves,
    traversal::{BlockContentItem, scan_block_content},
};
use table_cell::flatten_for_table_cell;
use tag_conversion::{convert_block, convert_paragraph};

mod markdown_text;
mod table_cell;
mod tag_conversion;
#[cfg(test)]
mod tests;

use markdown_text::{convert_to_markdown_text, render_block_content, render_list_table_cell};

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

/// Convert an already-structured Arguments section in the requested output format.
pub(crate) fn convert_arguments(
    arguments: &[RdArgument<'_>],
    format: crate::ArgumentsFormat,
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    match format {
        crate::ArgumentsFormat::PipeTable => convert_arguments_pipe(arguments, context),
        crate::ArgumentsFormat::GridTable => convert_arguments_grid(arguments, context),
        crate::ArgumentsFormat::ListTable => convert_arguments_list_table(arguments, context),
        crate::ArgumentsFormat::List => convert_arguments_list(arguments, context),
    }
}

/// Convert arguments to pipe table format.
/// Pipe tables cannot contain block elements (lists, multiple paragraphs).
/// Workaround: use `<br>` for line breaks and flatten lists with bullet markers.
fn convert_arguments_pipe(
    arguments: &[RdArgument<'_>],
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    if arguments.is_empty() {
        return Vec::new();
    }

    let header_row = Node::table_row(vec![
        Node::table_cell(vec![Node::text("Argument")]),
        Node::table_cell(vec![Node::text("Description")]),
    ]);
    let mut rows = vec![header_row];

    for argument in arguments {
        let term_text =
            replace_line_endings_with_space(&argument_name(argument, context)).replace('|', "\\|");
        rows.push(Node::table_row(vec![
            Node::table_cell(vec![Node::inline_code(term_text.trim())]),
            Node::table_cell(flatten_for_table_cell(argument.description, context)),
        ]));
    }

    vec![Node::table(
        vec![Some(Align::Left), Some(Align::Left)],
        rows,
    )]
}

/// Convert arguments to Pandoc grid table format.
/// Grid tables support block elements (lists, paragraphs) within cells.
fn convert_arguments_grid(
    arguments: &[RdArgument<'_>],
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    use tabled::builder::Builder;

    if arguments.is_empty() {
        return Vec::new();
    }

    let mut builder = Builder::default();
    builder.push_record(["Argument", "Description"]);

    for argument in arguments {
        let term_text = argument_name(argument, context);
        let arg_text = rd2qmd_mdast::format_inline_code(term_text.trim(), false);
        let desc_text = convert_to_markdown_text(argument.description, context);
        builder.push_record([arg_text, desc_text]);
    }

    let mut table = builder.build();
    let grid_style = Style::ascii().horizontals([(
        1,
        HorizontalLine::new('=')
            .left('+')
            .right('+')
            .intersection('+'),
    )]);
    let grid_table = table.with(grid_style).to_string();

    vec![Node::Html(Html { value: grid_table })]
}

/// Convert arguments to Quarto list-table format.
/// Requires Quarto 1.9+ and is compatible with q2.
fn convert_arguments_list_table(
    arguments: &[RdArgument<'_>],
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    if arguments.is_empty() {
        return Vec::new();
    }

    let rows: Vec<_> = arguments
        .iter()
        .map(|argument| {
            let term_text = argument_name(argument, context);
            let arg_text = rd2qmd_mdast::format_inline_code(term_text.trim(), false);
            let desc_nodes = convert_block_content(argument.description, context);
            let desc_text = render_list_table_cell(&desc_nodes);
            (arg_text, desc_text)
        })
        .collect();

    let mut output = String::new();
    output.push_str("::: {.list-table header-rows=1}\n\n");
    output.push_str("- - Argument\n  - Description\n");

    for (arg, desc) in &rows {
        output.push('\n');
        output.push_str("- - ");
        output.push_str(arg);
        output.push('\n');
        output.push_str("  - ");
        output.push_str(desc);
        output.push('\n');
    }

    output.push_str("\n:::\n");
    vec![Node::Html(Html { value: output })]
}

/// Convert arguments to Markdown loose-list format.
fn convert_arguments_list(
    arguments: &[RdArgument<'_>],
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    if arguments.is_empty() {
        return Vec::new();
    }

    let items: Vec<_> = arguments
        .iter()
        .map(|argument| {
            let term_text = argument_name(argument, context);
            let arg_code = rd2qmd_mdast::format_inline_code(term_text.trim(), false);
            let desc_nodes = convert_block_content(argument.description, context);
            let desc_text = render_block_content(&desc_nodes, 2);
            (arg_code, desc_text)
        })
        .collect();

    let mut output = String::new();
    for (i, (arg, desc)) in items.iter().enumerate() {
        if i > 0 {
            output.push('\n');
        }
        output.push_str("- **");
        output.push_str(arg);
        output.push_str("**\n");

        if !desc.is_empty() {
            output.push('\n');
            output.push_str("  ");
            output.push_str(desc);
            output.push('\n');
        }
    }

    vec![Node::Html(Html { value: output })]
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
