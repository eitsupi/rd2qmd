//! General block-content assembly for the rd_ast conversion migration.

use rd_ast::{RdListItem, RdListKind, RdNode, RdPath, RdTag};
use rd2qmd_mdast::Node;

use super::{
    inline::{self, LinkResolutionContext},
    leaf_text::flatten_verbatim_leaves,
    traversal::{BlockContentItem, ParagraphItem, scan_block_content},
};

/// Borrowed configuration used while converting general block content.
pub(crate) struct BlockConversionContext<'a> {
    pub(crate) links: LinkResolutionContext<'a>,
    pub(crate) prefer_ascii_math: bool,
}

/// Convert paragraphs and supported semantic blocks in source order.
pub(crate) fn convert_block_content(
    nodes: &[RdNode],
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    scan_block_content(nodes)
        .into_iter()
        .filter_map(|item| match item {
            BlockContentItem::Paragraph(items) => convert_paragraph(items, context),
            BlockContentItem::Block(node) => convert_block(node, context),
        })
        .collect()
}

fn convert_paragraph(
    items: Vec<ParagraphItem<'_>>,
    context: &BlockConversionContext<'_>,
) -> Option<Node> {
    let children: Vec<_> = items
        .into_iter()
        .filter_map(|item| match item {
            ParagraphItem::Text(text) => Some(inline::convert_text(text)),
            ParagraphItem::Node(node) => inline::convert_inline_node(node, &context.links),
        })
        .collect();

    (!children.is_empty()).then(|| Node::paragraph(children))
}

fn convert_block(node: &RdNode, context: &BlockConversionContext<'_>) -> Option<Node> {
    let tagged = node.as_tagged()?;
    let base_path = RdPath::new(Vec::new());

    match tagged.tag() {
        RdTag::Itemize | RdTag::Enumerate => {
            let list = tagged.inspect_list(&base_path).ok()?;
            let ordered = match list.kind() {
                RdListKind::Itemize => false,
                RdListKind::Enumerate => true,
                _ => return None,
            };

            // Recovery-first: a malformed item is skipped without discarding
            // valid items later in the same list.
            let items = list
                .items()
                .filter_map(|item| match item.ok()? {
                    RdListItem::Delimited(item) => {
                        Some(Node::list_item(convert_block_content(item.body(), context)))
                    }
                    _ => None,
                })
                .collect();
            Some(Node::list(ordered, items))
        }
        RdTag::Describe => {
            let list = tagged.inspect_list(&base_path).ok()?;
            if list.kind() != RdListKind::Describe {
                return None;
            }

            // Recovery-first: malformed described items are skipped while
            // subsequent structurally valid entries are still converted.
            let mut children = Vec::new();
            for item in list.items() {
                let Ok(RdListItem::Described(item)) = item else {
                    continue;
                };
                children.push(Node::definition_term(inline::convert_inline_nodes(
                    item.label(),
                    &context.links,
                )));
                // Unlike the legacy inline-only description, preserve arbitrary
                // block children such as multiple paragraphs and nested lists.
                children.push(Node::definition_description(convert_block_content(
                    item.body(),
                    context,
                )));
            }
            Some(Node::definition_list(children))
        }
        RdTag::Preformatted => Some(Node::code(None, recover_verbatim(tagged.children()))),
        RdTag::Deqn => {
            let equation = tagged.inspect_equation(&base_path).ok()?;
            if context.prefer_ascii_math
                && let Some(ascii) = equation.ascii()
            {
                let ascii = equation_text(ascii, &context.links);
                if !ascii.trim().is_empty() {
                    return Some(Node::code(None, ascii));
                }
            }
            Some(Node::math(equation_text(equation.latex(), &context.links)))
        }
        _ => None,
    }
}

fn recover_verbatim(nodes: &[RdNode]) -> String {
    flatten_verbatim_leaves(nodes).unwrap_or_else(|error| error.recovered_text().to_owned())
}

fn equation_text(nodes: &[RdNode], links: &LinkResolutionContext<'_>) -> String {
    inline::extract_plain_text(&inline::convert_inline_nodes(nodes, links))
}

#[cfg(test)]
mod tests {
    use rd_ast::{RdNode, RdTag};
    use rd2qmd_mdast::Node;

    use super::{BlockConversionContext, convert_block_content};
    use crate::convert_ast::inline::LinkResolutionContext;

    fn context(prefer_ascii_math: bool) -> BlockConversionContext<'static> {
        BlockConversionContext {
            links: LinkResolutionContext::default(),
            prefer_ascii_math,
        }
    }

    fn text(value: &str) -> RdNode {
        RdNode::Text(value.to_owned())
    }

    fn verb(value: &str) -> RdNode {
        RdNode::Verb(value.to_owned())
    }

    fn group(children: Vec<RdNode>) -> RdNode {
        RdNode::group(children)
    }

    fn tagged(tag: RdTag, children: Vec<RdNode>) -> RdNode {
        RdNode::tagged(tag, None, children)
    }

    fn item_marker() -> RdNode {
        tagged(RdTag::Item, vec![])
    }

    fn delimited_list(tag: RdTag, item_bodies: Vec<Vec<RdNode>>) -> RdNode {
        let children = item_bodies
            .into_iter()
            .flat_map(|body| std::iter::once(item_marker()).chain(body))
            .collect();
        tagged(tag, children)
    }

    fn described_item(label: Vec<RdNode>, body: Vec<RdNode>) -> RdNode {
        tagged(RdTag::Item, vec![group(label), group(body)])
    }

    fn equation(latex: &str, ascii: Option<&str>) -> RdNode {
        let mut children = vec![group(vec![verb(latex)])];
        if let Some(ascii) = ascii {
            children.push(group(vec![text(ascii)]));
        }
        tagged(RdTag::Deqn, children)
    }

    #[test]
    fn converts_paragraph_only_content() {
        assert_eq!(
            convert_block_content(&[text("plain body")], &context(false)),
            vec![Node::paragraph(vec![
                Node::text("plain"),
                Node::text(" "),
                Node::text("body"),
            ])]
        );
    }

    #[test]
    fn skips_paragraph_when_no_inline_nodes_are_convertible() {
        let unsupported = tagged(RdTag::Tabular, vec![]);
        assert!(convert_block_content(&[unsupported], &context(false)).is_empty());
    }

    #[test]
    fn converts_itemize_with_multiple_and_multi_paragraph_items() {
        let list = delimited_list(
            RdTag::Itemize,
            vec![
                vec![text("first paragraph\n\nsecond paragraph")],
                vec![text("another item")],
            ],
        );

        assert_eq!(
            convert_block_content(&[list], &context(false)),
            vec![Node::list(
                false,
                vec![
                    Node::list_item(vec![
                        Node::paragraph(vec![
                            Node::text("first"),
                            Node::text(" "),
                            Node::text("paragraph"),
                        ]),
                        Node::paragraph(vec![
                            Node::text("second"),
                            Node::text(" "),
                            Node::text("paragraph"),
                        ]),
                    ]),
                    Node::list_item(vec![Node::paragraph(vec![
                        Node::text("another"),
                        Node::text(" "),
                        Node::text("item"),
                    ])]),
                ],
            )]
        );
    }

    #[test]
    fn converts_enumerate_as_ordered_list() {
        let list = delimited_list(RdTag::Enumerate, vec![vec![text("one")], vec![text("two")]]);

        assert_eq!(
            convert_block_content(&[list], &context(false)),
            vec![Node::list(
                true,
                vec![
                    Node::list_item(vec![Node::paragraph(vec![Node::text("one")])]),
                    Node::list_item(vec![Node::paragraph(vec![Node::text("two")])]),
                ],
            )]
        );
    }

    #[test]
    fn skips_malformed_list_items_and_keeps_later_valid_items() {
        let list = tagged(
            RdTag::Itemize,
            vec![text("malformed"), item_marker(), text("valid")],
        );

        assert_eq!(
            convert_block_content(&[list], &context(false)),
            vec![Node::list(
                false,
                vec![Node::list_item(vec![Node::paragraph(vec![Node::text(
                    "valid"
                )])])],
            )]
        );
    }

    #[test]
    fn converts_describe_with_arbitrary_block_description_content() {
        let nested_list = delimited_list(RdTag::Itemize, vec![vec![text("nested item")]]);
        let describe = tagged(
            RdTag::Describe,
            vec![described_item(
                vec![tagged(RdTag::Code, vec![text("term")])],
                vec![text("description"), nested_list],
            )],
        );

        assert_eq!(
            convert_block_content(&[describe], &context(false)),
            vec![Node::definition_list(vec![
                Node::definition_term(vec![Node::inline_code("term")]),
                Node::definition_description(vec![
                    Node::paragraph(vec![Node::text("description")]),
                    Node::list(
                        false,
                        vec![Node::list_item(vec![Node::paragraph(vec![
                            Node::text("nested"),
                            Node::text(" "),
                            Node::text("item"),
                        ])])],
                    ),
                ]),
            ])]
        );
    }

    #[test]
    fn converts_preformatted_to_code_block_with_recovered_text() {
        let preformatted = tagged(
            RdTag::Preformatted,
            vec![verb("first\n"), text("recovered")],
        );

        assert_eq!(
            convert_block_content(&[preformatted], &context(false)),
            vec![Node::code(None, "first\nrecovered")]
        );
    }

    #[test]
    fn block_equation_uses_ascii_only_when_preferred_and_non_blank() {
        let with_ascii = equation("x^2", Some("x squared"));
        let blank_ascii = equation("y^2", Some("  \n"));

        assert_eq!(
            convert_block_content(std::slice::from_ref(&with_ascii), &context(true)),
            vec![Node::code(None, "x squared")]
        );
        assert_eq!(
            convert_block_content(&[with_ascii], &context(false)),
            vec![Node::math("x^2")]
        );
        assert_eq!(
            convert_block_content(&[blank_ascii], &context(true)),
            vec![Node::math("y^2")]
        );
    }

    #[test]
    fn interleaves_paragraphs_and_lists_in_source_order() {
        let list = delimited_list(RdTag::Itemize, vec![vec![text("item")]]);
        let nodes = vec![text("before\n\n"), list, text("\n\nafter")];

        assert_eq!(
            convert_block_content(&nodes, &context(false)),
            vec![
                Node::paragraph(vec![Node::text("before")]),
                Node::list(
                    false,
                    vec![Node::list_item(vec![Node::paragraph(vec![Node::text(
                        "item"
                    )])])],
                ),
                Node::paragraph(vec![Node::text("after")]),
            ]
        );
    }
}
