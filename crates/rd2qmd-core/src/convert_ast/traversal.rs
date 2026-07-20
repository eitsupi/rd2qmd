//! Source-order paragraph boundary scanning for rd_ast sibling nodes.

/// Scan siblings into paragraphs, retaining visible source nodes in order.
///
/// Whitespace is held until the next visible item. A boundary requires at
/// least two newlines in whitespace/comments after visible content. Tagged
/// nodes are atomic and are not inspected for internal newlines.
///
/// Block classification is conservative until semantic dispatch is added;
/// currently no tagged node is classified as a block.
pub(crate) fn scan_paragraphs(nodes: &[rd_ast::RdNode]) -> Vec<Vec<&rd_ast::RdNode>> {
    let mut paragraphs = Vec::new();
    scan(nodes, &mut paragraphs);
    paragraphs
}

fn scan<'a>(nodes: &'a [rd_ast::RdNode], paragraphs: &mut Vec<Vec<&'a rd_ast::RdNode>>) {
    let mut current = Vec::new();
    let mut pending_whitespace = String::new();
    let mut has_visible = false;

    for node in nodes {
        match node {
            rd_ast::RdNode::Text(value) => {
                for part in value.split_inclusive(char::is_whitespace) {
                    if part.chars().all(char::is_whitespace) {
                        pending_whitespace.push_str(part);
                    } else {
                        let visible = part.trim_end_matches(char::is_whitespace);
                        append_visible(
                            node,
                            &mut current,
                            &mut pending_whitespace,
                            &mut has_visible,
                            paragraphs,
                        );
                        pending_whitespace.push_str(&part[visible.len()..]);
                    }
                }
            }
            rd_ast::RdNode::Comment(_) => {}
            rd_ast::RdNode::Group(group) => scan(group.children(), paragraphs),
            rd_ast::RdNode::Raw(raw) => scan(raw.children(), paragraphs),
            rd_ast::RdNode::Tagged(_) if is_block_level(node) => {
                flush(
                    &mut current,
                    &mut pending_whitespace,
                    &mut has_visible,
                    paragraphs,
                );
            }
            rd_ast::RdNode::Tagged(_) | rd_ast::RdNode::RCode(_) | rd_ast::RdNode::Verb(_) => {
                append_visible(
                    node,
                    &mut current,
                    &mut pending_whitespace,
                    &mut has_visible,
                    paragraphs,
                );
            }
            _ => {}
        }
    }
    flush(
        &mut current,
        &mut pending_whitespace,
        &mut has_visible,
        paragraphs,
    );
}

fn append_visible<'a>(
    node: &'a rd_ast::RdNode,
    current: &mut Vec<&'a rd_ast::RdNode>,
    pending: &mut String,
    has_visible: &mut bool,
    paragraphs: &mut Vec<Vec<&'a rd_ast::RdNode>>,
) {
    if pending.matches('\n').count() >= 2 && *has_visible {
        flush(current, pending, has_visible, paragraphs);
    } else {
        pending.clear();
    }
    if !current.contains(&node) {
        current.push(node);
    }
    *has_visible = true;
}

fn flush<'a>(
    current: &mut Vec<&'a rd_ast::RdNode>,
    pending: &mut String,
    has_visible: &mut bool,
    paragraphs: &mut Vec<Vec<&'a rd_ast::RdNode>>,
) {
    pending.clear();
    if *has_visible {
        paragraphs.push(std::mem::take(current));
    }
    *has_visible = false;
}

fn is_block_level(_node: &rd_ast::RdNode) -> bool {
    // TODO: classify semantic block tags when block conversion is introduced.
    false
}

#[cfg(test)]
mod tests {
    use super::scan_paragraphs;

    #[test]
    fn finds_two_paragraphs() {
        let parsed = rd_source::parse(b"first\n\nsecond").unwrap();
        assert_eq!(scan_paragraphs(parsed.document().nodes()).len(), 2);
    }

    #[test]
    fn comments_do_not_break_blank_line_detection() {
        let parsed = rd_source::parse(b"first\n% comment\n\nsecond").unwrap();
        assert_eq!(scan_paragraphs(parsed.document().nodes()).len(), 2);
    }

    #[test]
    fn whitespace_only_input_has_no_paragraphs() {
        let parsed = rd_source::parse(b" \n\n\t").unwrap();
        assert!(scan_paragraphs(parsed.document().nodes()).is_empty());
    }
}
