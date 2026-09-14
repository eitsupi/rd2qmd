//! The actual Rd-tag-to-block-node conversion: turning a scanned paragraph or
//! tagged block node into mdast [`Node`]s.

use rd_ast::{RdListItem, RdListKind, RdNodeRef, RdTag};
use rd2qmd_mdast::{Align, Node};

use crate::convert_ast::inline::{self, InlineConversionContext};
use crate::convert_ast::traversal::ParagraphItem;

use super::table_cell::sanitize_table_cell_inline_nodes;
use super::{BlockConversionContext, recover_verbatim};

pub(super) fn convert_paragraph(
    items: Vec<ParagraphItem<'_>>,
    context: &BlockConversionContext<'_>,
) -> Option<Node> {
    let children: Vec<_> = items
        .into_iter()
        .flat_map(|item| match item {
            ParagraphItem::Text(text) => vec![inline::convert_text(text)],
            ParagraphItem::Node(node) => inline::convert_inline_node_ref(node, &context.inline)
                .into_iter()
                .collect(),
        })
        .collect();

    (!children.is_empty()).then(|| Node::paragraph(children))
}

pub(super) fn convert_block(
    node: RdNodeRef<'_>,
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    let Some(tagged) = node.node().as_tagged() else {
        return Vec::new();
    };

    match tagged.tag() {
        RdTag::Itemize | RdTag::Enumerate => {
            let Some(list) = node.inspect_list().ok().flatten() else {
                return Vec::new();
            };
            let ordered = match list.kind() {
                RdListKind::Itemize => false,
                RdListKind::Enumerate => true,
                _ => return Vec::new(),
            };

            // Recovery-first: a malformed item is skipped without discarding
            // valid items later in the same list.
            let items = list
                .items()
                .filter_map(|item| match item.ok()? {
                    RdListItem::Delimited(item) => Some(Node::list_item(
                        super::convert_block_content_ref(item.body_ref(), context),
                    )),
                    _ => None,
                })
                .collect();
            vec![Node::list(ordered, items)]
        }
        RdTag::Describe => {
            let Some(list) = node.inspect_list().ok().flatten() else {
                return Vec::new();
            };
            if list.kind() != RdListKind::Describe {
                return Vec::new();
            }

            // Recovery-first: malformed described items are skipped while
            // subsequent structurally valid entries are still converted.
            let mut children = Vec::new();
            for item in list.items() {
                let Ok(RdListItem::Described(item)) = item else {
                    continue;
                };
                children.push(Node::definition_term(inline::convert_inline_nodes_ref(
                    item.label_ref(),
                    &context.inline,
                )));
                // Unlike the legacy inline-only description, preserve arbitrary
                // block children such as multiple paragraphs and nested lists.
                children.push(Node::definition_description(
                    super::convert_block_content_ref(item.body_ref(), context),
                ));
            }
            vec![Node::definition_list(children)]
        }
        RdTag::Preformatted => vec![Node::code(None, recover_verbatim(tagged.children()))],
        RdTag::Deqn => {
            let Some(equation) = node.inspect_equation().ok().flatten() else {
                return Vec::new();
            };
            if context.prefer_ascii_math
                && let Some(ascii) = equation.ascii()
            {
                let ascii = recover_verbatim(ascii);
                if !ascii.trim().is_empty() {
                    return vec![Node::code(None, ascii)];
                }
            }
            vec![Node::math(equation_text_ref(
                equation.latex_ref(),
                &context.inline,
            ))]
        }
        RdTag::Tabular => convert_tabular(node, context).into_iter().collect(),
        RdTag::Section | RdTag::Subsection => convert_section_like_block(node, context),
        _ => Vec::new(),
    }
}

fn convert_tabular(node: RdNodeRef<'_>, context: &BlockConversionContext<'_>) -> Option<Node> {
    let table = node.inspect_tabular().ok()??;
    // rd-ast skips unrecognized colspec characters, whereas legacy conversion
    // retained an unaligned placeholder. This can shift alignment for malformed
    // specs; rows remain recovery-safe because the GFM writer pads ragged rows.
    let align = table
        .columns()
        .iter()
        .map(|column| match column {
            rd_ast::RdColumnAlign::Left => Some(Align::Left),
            rd_ast::RdColumnAlign::Center => Some(Align::Center),
            rd_ast::RdColumnAlign::Right => Some(Align::Right),
            _ => None,
        })
        .collect();
    let rows = table
        .rows()
        .iter()
        .map(|row| {
            let cells = row
                .cells()
                .iter()
                .map(|cell| {
                    let children = sanitize_table_cell_inline_nodes(
                        &inline::convert_inline_nodes_ref(cell.nodes_ref(), &context.inline),
                    );
                    Node::table_cell(children)
                })
                .collect();
            Node::table_row(cells)
        })
        .collect();
    Some(Node::table(align, rows))
}

fn convert_section_like_block(
    node: RdNodeRef<'_>,
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    let tagged = node.node().as_tagged().expect("section node is tagged");
    if tagged.option().is_some() {
        return Vec::new();
    }
    let [title, body] = tagged.children() else {
        return Vec::new();
    };
    if title.as_group().is_none() || body.as_group().is_none() {
        return Vec::new();
    }
    let depth = context.enclosing_heading_depth.saturating_add(1).min(6);
    let children = node.children();
    let title_ref = children.get(0).expect("section title exists").children();
    let body_ref = children.get(1).expect("section body exists").children();
    let mut nodes = vec![Node::heading(
        depth,
        inline::convert_inline_nodes_ref(title_ref, &context.inline),
    )];
    let child_context = BlockConversionContext {
        enclosing_heading_depth: depth,
        ..*context
    };
    nodes.extend(super::convert_block_content_ref(body_ref, &child_context));
    nodes
}

fn equation_text_ref(
    nodes: rd_ast::RdNodesRef<'_>,
    context: &InlineConversionContext<'_>,
) -> String {
    inline::extract_plain_text(&inline::convert_inline_nodes_ref(nodes, context))
}
