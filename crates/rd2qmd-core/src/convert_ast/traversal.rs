//! Source-order block-content scanning for rd_ast sibling nodes.

use rd_ast::RdTag;

#[cfg(feature = "roxygen")]
use super::roxygen::{RoxygenCodeBlock, try_match_roxygen_code_block};

/// A borrowed piece of source content belonging to one paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ParagraphItem<'a> {
    Node(&'a rd_ast::RdNode),
    Text(&'a str),
}

/// One paragraph or semantic block retained in source order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BlockContentItem<'a> {
    Paragraph(Vec<ParagraphItem<'a>>),
    Block(&'a rd_ast::RdNode),
    #[cfg(feature = "roxygen")]
    RoxygenCode(RoxygenCodeBlock),
}

/// Scan siblings into paragraphs and semantic blocks in source order.
///
/// Whitespace is held until the next visible item. A boundary requires at
/// least two newlines in whitespace/comments after visible content. Tagged
/// nodes are atomic and are not inspected for internal newlines.
pub(crate) fn scan_block_content(nodes: &[rd_ast::RdNode]) -> Vec<BlockContentItem<'_>> {
    let mut items = Vec::new();
    let mut state = ScanState::default();
    scan(nodes, &mut state, &mut items);
    flush(&mut state, &mut items);
    items
}

#[derive(Default)]
struct ScanState<'a> {
    current: Vec<ParagraphItem<'a>>,
    pending_whitespace: Vec<&'a str>,
    pending_newlines: usize,
    has_visible: bool,
}

fn scan<'a>(
    nodes: &'a [rd_ast::RdNode],
    state: &mut ScanState<'a>,
    items: &mut Vec<BlockContentItem<'a>>,
) {
    let mut cursor = 0;
    while cursor < nodes.len() {
        #[cfg(feature = "roxygen")]
        if let Some(block) = try_match_roxygen_code_block(&nodes[cursor..]) {
            flush(state, items);
            items.push(BlockContentItem::RoxygenCode(block));
            cursor += 3;
            continue;
        }

        let node = &nodes[cursor];
        match node {
            rd_ast::RdNode::Text(value) => {
                for part in value.split_inclusive(char::is_whitespace) {
                    if part.chars().all(char::is_whitespace) {
                        append_whitespace(part, state);
                    } else {
                        let visible = part.trim_end_matches(char::is_whitespace);
                        append_visible(ParagraphItem::Text(visible), state, items);
                        append_whitespace(&part[visible.len()..], state);
                    }
                }
            }
            rd_ast::RdNode::Comment(_) => {}
            rd_ast::RdNode::Group(group) => scan(group.children(), state, items),
            rd_ast::RdNode::Raw(raw) => scan(raw.children(), state, items),
            rd_ast::RdNode::Tagged(tagged) if matches!(tagged.tag(), RdTag::Unknown(_)) => {
                scan(tagged.children(), state, items)
            }
            rd_ast::RdNode::Tagged(_) if is_block_level(node) => {
                flush(state, items);
                items.push(BlockContentItem::Block(node));
            }
            rd_ast::RdNode::Tagged(_) | rd_ast::RdNode::RCode(_) | rd_ast::RdNode::Verb(_) => {
                append_visible(ParagraphItem::Node(node), state, items);
            }
            _ => {}
        }
        cursor += 1;
    }
}

fn append_whitespace<'a>(whitespace: &'a str, state: &mut ScanState<'a>) {
    if !whitespace.is_empty() {
        state.pending_newlines += whitespace.matches('\n').count();
        state.pending_whitespace.push(whitespace);
    }
}

fn append_visible<'a>(
    item: ParagraphItem<'a>,
    state: &mut ScanState<'a>,
    items: &mut Vec<BlockContentItem<'a>>,
) {
    if state.has_visible {
        if state.pending_newlines >= 2 {
            flush(state, items);
        } else {
            state
                .current
                .extend(state.pending_whitespace.drain(..).map(ParagraphItem::Text));
            state.pending_newlines = 0;
        }
    } else {
        state.pending_whitespace.clear();
        state.pending_newlines = 0;
    }
    state.current.push(item);
    state.has_visible = true;
}

fn flush<'a>(state: &mut ScanState<'a>, items: &mut Vec<BlockContentItem<'a>>) {
    state.pending_whitespace.clear();
    state.pending_newlines = 0;
    if state.has_visible {
        items.push(BlockContentItem::Paragraph(std::mem::take(
            &mut state.current,
        )));
    }
    state.has_visible = false;
}

fn is_block_level(node: &rd_ast::RdNode) -> bool {
    node.as_tagged().is_some_and(|tagged| {
        matches!(
            tagged.tag(),
            RdTag::Itemize
                | RdTag::Enumerate
                | RdTag::Describe
                | RdTag::Preformatted
                | RdTag::Deqn
                | RdTag::Tabular
                | RdTag::Section
                | RdTag::Subsection
        )
    })
}

#[cfg(test)]
mod tests {
    use rd_ast::{RdNode, RdTag, producer};

    use super::{BlockContentItem, ParagraphItem, scan_block_content};

    fn paragraph_text(items: &[ParagraphItem<'_>]) -> String {
        items
            .iter()
            .map(|item| match item {
                ParagraphItem::Text(text) => *text,
                ParagraphItem::Node(rd_ast::RdNode::RCode(text))
                | ParagraphItem::Node(rd_ast::RdNode::Verb(text)) => text,
                ParagraphItem::Node(node) => panic!("unexpected whole node: {node:?}"),
            })
            .collect()
    }

    #[test]
    fn finds_two_paragraphs() {
        let nodes = vec![RdNode::Text("first\n\nsecond".into())];
        let items = scan_block_content(&nodes);
        let [
            BlockContentItem::Paragraph(first),
            BlockContentItem::Paragraph(second),
        ] = items.as_slice()
        else {
            panic!("expected two paragraphs");
        };
        assert_eq!(paragraph_text(first), "first");
        assert_eq!(paragraph_text(second), "second");
    }

    #[test]
    fn comments_do_not_break_blank_line_detection() {
        let nodes = vec![
            RdNode::Text("first\n".into()),
            RdNode::Comment("% comment".into()),
            RdNode::Text("\nsecond".into()),
        ];
        let items = scan_block_content(&nodes);
        let [
            BlockContentItem::Paragraph(first),
            BlockContentItem::Paragraph(second),
        ] = items.as_slice()
        else {
            panic!("expected two paragraphs");
        };
        assert_eq!(paragraph_text(first), "first");
        assert_eq!(paragraph_text(second), "second");
    }

    #[test]
    fn whitespace_only_input_has_no_paragraphs() {
        let nodes = vec![RdNode::Text(" \n\n\t".into())];
        assert!(scan_block_content(&nodes).is_empty());
    }

    #[test]
    fn group_children_stay_in_the_enclosing_paragraph() {
        let nodes = vec![
            rd_ast::RdNode::Text("before ".to_string()),
            rd_ast::RdNode::group(vec![rd_ast::RdNode::Text("inside".to_string())]),
            rd_ast::RdNode::Text(" after".to_string()),
        ];

        let items = scan_block_content(&nodes);
        let [BlockContentItem::Paragraph(paragraph)] = items.as_slice() else {
            panic!("expected one paragraph");
        };
        assert_eq!(paragraph_text(paragraph), "before inside after");
    }

    #[test]
    fn structurally_equal_siblings_are_both_retained() {
        let nodes = vec![
            rd_ast::RdNode::RCode("same".to_string()),
            rd_ast::RdNode::RCode("same".to_string()),
        ];

        let items = scan_block_content(&nodes);
        let [BlockContentItem::Paragraph(paragraph)] = items.as_slice() else {
            panic!("expected one paragraph");
        };
        assert_eq!(paragraph.len(), 2);
        assert_eq!(paragraph_text(paragraph), "samesame");
        assert!(matches!(
            paragraph.as_slice(),
            [ParagraphItem::Node(first), ParagraphItem::Node(second)]
                if std::ptr::eq(*first, &nodes[0]) && std::ptr::eq(*second, &nodes[1])
        ));
    }

    #[test]
    fn retains_block_between_surrounding_paragraphs() {
        let nodes = vec![
            RdNode::Text("before\n\n".to_owned()),
            RdNode::tagged(RdTag::Itemize, None, vec![]),
            RdNode::Text("\n\nafter".to_owned()),
        ];

        let items = scan_block_content(&nodes);
        assert!(matches!(
            items.as_slice(),
            [
                BlockContentItem::Paragraph(before),
                BlockContentItem::Block(block),
                BlockContentItem::Paragraph(after),
            ] if paragraph_text(before) == "before"
                && std::ptr::eq(*block, &nodes[1])
                && paragraph_text(after) == "after"
        ));
    }

    #[test]
    fn unknown_tag_wrapper_does_not_merge_blank_line_separated_paragraphs() {
        let nodes = vec![RdNode::tagged(
            RdTag::Unknown(r"\madeUpTag".into()),
            None,
            vec![RdNode::Text("first\n\nsecond".into())],
        )];
        let items = scan_block_content(&nodes);
        let [
            BlockContentItem::Paragraph(first),
            BlockContentItem::Paragraph(second),
        ] = items.as_slice()
        else {
            panic!("expected two paragraphs, got {items:?}");
        };
        assert_eq!(paragraph_text(first), "first");
        assert_eq!(paragraph_text(second), "second");
    }

    #[test]
    fn unknown_tag_wrapper_exposes_nested_block_content() {
        let nodes = vec![RdNode::tagged(
            RdTag::Unknown(r"\madeUpTag".into()),
            None,
            vec![RdNode::tagged(
                RdTag::Itemize,
                None,
                vec![RdNode::tagged(
                    RdTag::Item,
                    None,
                    vec![RdNode::Text("a".into())],
                )],
            )],
        )];
        let items = scan_block_content(&nodes);
        let [BlockContentItem::Block(block)] = items.as_slice() else {
            panic!("expected the nested itemize to surface as a block, got {items:?}");
        };
        assert_eq!(block.as_tagged().unwrap().tag(), &RdTag::Itemize);
    }

    #[test]
    fn finds_blocks_nested_in_group_and_raw_wrappers() {
        let grouped_block = RdNode::tagged(RdTag::Preformatted, None, vec![]);
        let raw_block = RdNode::tagged(RdTag::Deqn, None, vec![]);
        let nodes = vec![
            RdNode::group(vec![grouped_block]),
            RdNode::Raw(producer::raw_node(
                None,
                None,
                vec![raw_block],
                None,
                vec![],
            )),
        ];

        let items = scan_block_content(&nodes);
        assert!(matches!(
            items.as_slice(),
            [BlockContentItem::Block(first), BlockContentItem::Block(second)]
                if first.as_tagged().unwrap().tag() == &RdTag::Preformatted
                    && second.as_tagged().unwrap().tag() == &RdTag::Deqn
        ));
    }
}
