//! Source-order paragraph boundary scanning for rd_ast sibling nodes.

/// A borrowed piece of source content belonging to one paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ParagraphItem<'a> {
    Node(&'a rd_ast::RdNode),
    Text(&'a str),
}

/// Scan siblings into paragraphs, retaining visible source nodes in order.
///
/// Whitespace is held until the next visible item. A boundary requires at
/// least two newlines in whitespace/comments after visible content. Tagged
/// nodes are atomic and are not inspected for internal newlines.
///
/// Block classification is conservative until semantic dispatch is added;
/// currently no tagged node is classified as a block.
pub(crate) fn scan_paragraphs(nodes: &[rd_ast::RdNode]) -> Vec<Vec<ParagraphItem<'_>>> {
    let mut paragraphs = Vec::new();
    let mut state = ScanState::default();
    scan(nodes, &mut state, &mut paragraphs);
    flush(&mut state, &mut paragraphs);
    paragraphs
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
    paragraphs: &mut Vec<Vec<ParagraphItem<'a>>>,
) {
    for node in nodes {
        match node {
            rd_ast::RdNode::Text(value) => {
                for part in value.split_inclusive(char::is_whitespace) {
                    if part.chars().all(char::is_whitespace) {
                        append_whitespace(part, state);
                    } else {
                        let visible = part.trim_end_matches(char::is_whitespace);
                        append_visible(ParagraphItem::Text(visible), state, paragraphs);
                        append_whitespace(&part[visible.len()..], state);
                    }
                }
            }
            rd_ast::RdNode::Comment(_) => {}
            rd_ast::RdNode::Group(group) => scan(group.children(), state, paragraphs),
            rd_ast::RdNode::Raw(raw) => scan(raw.children(), state, paragraphs),
            rd_ast::RdNode::Tagged(_) if is_block_level(node) => {
                flush(state, paragraphs);
            }
            rd_ast::RdNode::Tagged(_) | rd_ast::RdNode::RCode(_) | rd_ast::RdNode::Verb(_) => {
                append_visible(ParagraphItem::Node(node), state, paragraphs);
            }
            _ => {}
        }
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
    paragraphs: &mut Vec<Vec<ParagraphItem<'a>>>,
) {
    if state.has_visible {
        if state.pending_newlines >= 2 {
            flush(state, paragraphs);
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

fn flush<'a>(state: &mut ScanState<'a>, paragraphs: &mut Vec<Vec<ParagraphItem<'a>>>) {
    state.pending_whitespace.clear();
    state.pending_newlines = 0;
    if state.has_visible {
        paragraphs.push(std::mem::take(&mut state.current));
    }
    state.has_visible = false;
}

fn is_block_level(_node: &rd_ast::RdNode) -> bool {
    // TODO: classify semantic block tags when block conversion is introduced.
    false
}

#[cfg(test)]
mod tests {
    use super::{ParagraphItem, scan_paragraphs};

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
        let parsed = rd_source::parse(b"first\n\nsecond").unwrap();
        let paragraphs = scan_paragraphs(parsed.document().nodes());
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraph_text(&paragraphs[0]), "first");
        assert_eq!(paragraph_text(&paragraphs[1]), "second");
    }

    #[test]
    fn comments_do_not_break_blank_line_detection() {
        let parsed = rd_source::parse(b"first\n% comment\n\nsecond").unwrap();
        let paragraphs = scan_paragraphs(parsed.document().nodes());
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraph_text(&paragraphs[0]), "first");
        assert_eq!(paragraph_text(&paragraphs[1]), "second");
    }

    #[test]
    fn whitespace_only_input_has_no_paragraphs() {
        let parsed = rd_source::parse(b" \n\n\t").unwrap();
        assert!(scan_paragraphs(parsed.document().nodes()).is_empty());
    }

    #[test]
    fn group_children_stay_in_the_enclosing_paragraph() {
        let nodes = vec![
            rd_ast::RdNode::Text("before ".to_string()),
            rd_ast::RdNode::group(vec![rd_ast::RdNode::Text("inside".to_string())]),
            rd_ast::RdNode::Text(" after".to_string()),
        ];

        let paragraphs = scan_paragraphs(&nodes);
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraph_text(&paragraphs[0]), "before inside after");
    }

    #[test]
    fn structurally_equal_siblings_are_both_retained() {
        let nodes = vec![
            rd_ast::RdNode::RCode("same".to_string()),
            rd_ast::RdNode::RCode("same".to_string()),
        ];

        let paragraphs = scan_paragraphs(&nodes);
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0].len(), 2);
        assert_eq!(paragraph_text(&paragraphs[0]), "samesame");
        assert!(matches!(
            paragraphs[0].as_slice(),
            [ParagraphItem::Node(first), ParagraphItem::Node(second)]
                if std::ptr::eq(*first, &nodes[0]) && std::ptr::eq(*second, &nodes[1])
        ));
    }
}
