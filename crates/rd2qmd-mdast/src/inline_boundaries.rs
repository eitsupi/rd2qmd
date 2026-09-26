//! Structural whitespace normalization before Markdown serialization.

use crate::Node;

/// Move boundary whitespace out of emphasis and strong spans, without deleting
/// it. Empty formatting spans disappear. Links retain their boundary whitespace
/// inside the link; code, math, HTML, images and breaks are opaque.
///
/// Empty definition terms use a zero-width character reference so their
/// descriptions remain inside the definition list after parsing Markdown.
/// Leading empty list-item paragraphs likewise retain a nonempty placeholder.
/// Block containers are traversed without changing their membership. This makes
/// the same operation usable for a complete document or an isolated inline
/// sequence. The input is borrowed, and normalization is idempotent.
///
/// This handles whitespace boundaries, not general Markdown delimiter selection
/// (such as intraword underscores or adjacent emphasis runs).
pub fn normalize_inline_boundaries(nodes: &[Node]) -> Vec<Node> {
    normalize_owned(nodes.to_vec())
}

fn normalize_owned(nodes: Vec<Node>) -> Vec<Node> {
    let mut result = Vec::with_capacity(nodes.len());
    for mut node in nodes {
        let decorated = matches!(node, Node::Emphasis(_) | Node::Strong(_));
        let children = match &mut node {
            Node::Heading(n) => Some(&mut n.children),
            Node::Paragraph(n) => Some(&mut n.children),
            Node::Blockquote(n) => Some(&mut n.children),
            Node::List(n) => Some(&mut n.children),
            Node::ListItem(n) => Some(&mut n.children),
            Node::Table(n) => Some(&mut n.children),
            Node::TableRow(n) => Some(&mut n.children),
            Node::TableCell(n) => Some(&mut n.children),
            Node::DefinitionList(n) => Some(&mut n.children),
            Node::DefinitionTerm(n) => Some(&mut n.children),
            Node::DefinitionDescription(n) => Some(&mut n.children),
            Node::Emphasis(n) => Some(&mut n.children),
            Node::Strong(n) => Some(&mut n.children),
            Node::Link(n) => Some(&mut n.children),
            Node::Text(_)
            | Node::Code(_)
            | Node::InlineCode(_)
            | Node::Math(_)
            | Node::InlineMath(_)
            | Node::Html(_)
            | Node::Image(_)
            | Node::Break
            | Node::ThematicBreak => None,
        };
        let mut suffix = String::new();
        if let Some(children) = children {
            *children = normalize_owned(std::mem::take(children));
            if decorated {
                if let Some(Node::Text(text)) = children.first_mut() {
                    let boundary =
                        text.value.len() - text.value.trim_start_matches(is_whitespace).len();
                    let rest = text.value.split_off(boundary);
                    append(
                        &mut result,
                        Node::text(std::mem::replace(&mut text.value, rest)),
                    );
                    if text.value.is_empty() {
                        children.remove(0);
                    }
                }
                if let Some(Node::Text(text)) = children.last_mut() {
                    let boundary = text.value.trim_end_matches(is_whitespace).len();
                    suffix = text.value.split_off(boundary);
                    if text.value.is_empty() {
                        children.pop();
                    }
                }
                if children.is_empty() {
                    append(&mut result, Node::text(suffix));
                    continue;
                }
            }
        }
        match &mut node {
            Node::DefinitionTerm(term) if blank_text(&term.children) => {
                term.children = vec![empty_term()];
            }
            Node::ListItem(item) if item.children.len() > 1 => {
                if let Node::Paragraph(first) = &mut item.children[0]
                    && blank_text(&first.children)
                {
                    first.children = vec![empty_term()];
                }
            }
            _ => {}
        }
        append(&mut result, node);
        append(&mut result, Node::text(suffix));
    }
    result
}

fn blank_text(nodes: &[Node]) -> bool {
    nodes
        .iter()
        .all(|node| matches!(node, Node::Text(text) if text.value.chars().all(is_whitespace)))
}

fn empty_term() -> Node {
    // Character references work in Pandoc and CommonMark without raw HTML
    // support. U+200B supplies a term without inventing a visible label.
    Node::Html(crate::Html {
        value: "&#8203;".to_owned(),
    })
}

fn append(nodes: &mut Vec<Node>, node: Node) {
    if let Node::Text(text) = &node {
        if text.value.is_empty() {
            return;
        }
        if let Some(Node::Text(previous)) = nodes.last_mut() {
            previous.value.push_str(&text.value);
            return;
        }
    }
    nodes.push(node);
}

// CommonMark Unicode whitespace: Zs plus TAB, LF, FF and CR. Rust's
// char::is_whitespace additionally includes VT, NEL, line/paragraph separators.
fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

#[cfg(test)]
mod tests;
