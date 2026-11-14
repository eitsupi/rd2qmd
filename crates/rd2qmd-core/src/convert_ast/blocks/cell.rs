//! Rendering of Arguments cell content (list-table, list and grid-table
//! formats) through the mdast writer.
//!
//! Cells are isolated block fragments embedded in a larger hand-built
//! construct, so the nodes are prepared, rendered with
//! [`render_blocks_fragment`], and re-indented under the construct's marker.

use rd2qmd_mdast::{
    DefinitionListSyntax, Node, WriterOptions, normalize_inline_boundaries, render_blocks_fragment,
};

/// Serialize one node through the real writer.
///
/// Used only for nodes flattened into pipe-table cells (code, math,
/// headings), which never contain a definition list.
pub(super) fn node_to_markdown_string(node: &Node) -> String {
    render_blocks_fragment(
        std::slice::from_ref(node),
        &writer_options(DefinitionListSyntax::default()),
    )
}

fn writer_options(definition_list_syntax: DefinitionListSyntax) -> WriterOptions {
    WriterOptions {
        frontmatter: None,
        // The migration context has no writer-format option yet. Preserve the
        // legacy call site's default until the document converter is wired.
        quarto_code_blocks: true,
        definition_list_syntax,
    }
}

/// Render block nodes as a cell body whose continuation lines are indented by
/// `indent` spaces. The first line is not indented: the caller places it after
/// its own marker. Empty content yields an empty string.
///
/// Cells are rendered during conversion, before the document writer runs, so
/// the definition list syntax must match the one the document uses.
pub(super) fn render_cell(
    nodes: &[Node],
    indent: usize,
    definition_list_syntax: DefinitionListSyntax,
) -> String {
    let mut nodes = normalize_inline_boundaries(nodes);
    prepare_blocks(&mut nodes, true);
    let rendered = render_blocks_fragment(&nodes, &writer_options(definition_list_syntax));
    indent_continuation(&rendered, indent)
}

/// Prefix every non-empty line after the first with `indent` spaces.
fn indent_continuation(text: &str, indent: usize) -> String {
    let prefix = " ".repeat(indent);
    let mut result = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            result.push('\n');
            if !line.is_empty() {
                result.push_str(&prefix);
            }
        }
        result.push_str(line);
    }
    result
}

fn is_blank_paragraph(paragraph: &rd2qmd_mdast::Paragraph) -> bool {
    paragraph
        .children
        .iter()
        .all(|child| matches!(child, Node::Text(text) if text.value.trim().is_empty()))
}

/// Drop blank paragraphs and normalize paragraph whitespace.
///
/// `drop_first` is false for the first child of a list item: an empty leading
/// paragraph there carries the writer's placeholder that keeps the item
/// non-empty.
fn prepare_blocks(nodes: &mut Vec<Node>, drop_first: bool) {
    let mut index = 0;
    nodes.retain(|node| {
        let keep = match node {
            Node::Paragraph(paragraph) => {
                (index == 0 && !drop_first) || !is_blank_paragraph(paragraph)
            }
            _ => true,
        };
        index += 1;
        keep
    });
    for node in nodes.iter_mut() {
        match node {
            Node::Paragraph(paragraph) => prepare_paragraph(&mut paragraph.children),
            Node::List(list) => {
                for item in &mut list.children {
                    if let Node::ListItem(item) = item {
                        prepare_blocks(&mut item.children, false);
                    }
                }
            }
            // Trailing spaces in a heading are insignificant.
            Node::Heading(heading) => {
                if let Some(Node::Text(text)) = heading.children.last_mut() {
                    text.value.truncate(text.value.trim_end().len());
                    if text.value.is_empty() {
                        heading.children.pop();
                    }
                }
            }
            // Leading spaces before the first term are insignificant.
            Node::DefinitionList(list) => {
                if let Some(Node::DefinitionTerm(term)) = list.children.first_mut()
                    && let Some(Node::Text(text)) = term.children.first_mut()
                {
                    text.value = text.value.trim_start().to_owned();
                }
            }
            _ => {}
        }
    }
}

/// Trim leading whitespace at the start of a paragraph and after every line
/// break, then escape list-marker lookalikes at those line starts.
///
/// Spaces at the start of a line are insignificant in Markdown, but a line
/// that begins like a list marker would otherwise open a list. This is
/// interim: it goes away once the writer escapes Markdown text itself.
fn prepare_paragraph(children: &mut [Node]) {
    let mut at_line_start = true;
    let mut is_paragraph_start = true;
    prepare_inline(children, &mut at_line_start, &mut is_paragraph_start);
}

fn prepare_inline(nodes: &mut [Node], at_line_start: &mut bool, is_paragraph_start: &mut bool) {
    for node in nodes {
        match node {
            Node::Text(text) => {
                let mut result = String::with_capacity(text.value.len());
                for (i, segment) in text.value.split('\n').enumerate() {
                    if i > 0 {
                        result.push('\n');
                        *at_line_start = true;
                        *is_paragraph_start = false;
                    }
                    if *at_line_start && !segment.trim_start().is_empty() {
                        let trimmed = segment.trim_start();
                        // Paragraph starts are trimmed but, for now, not
                        // escaped; only lines after a break are.
                        if *is_paragraph_start {
                            result.push_str(trimmed);
                        } else {
                            result.push_str(&escape_list_marker(trimmed));
                        }
                        *at_line_start = false;
                        *is_paragraph_start = false;
                    } else if *at_line_start {
                        // Whitespace-only segment: stay at the line start.
                    } else {
                        result.push_str(segment);
                    }
                }
                text.value = result;
            }
            Node::Break => {
                *at_line_start = true;
                *is_paragraph_start = false;
            }
            Node::Emphasis(n) => prepare_inline(&mut n.children, at_line_start, is_paragraph_start),
            Node::Strong(n) => prepare_inline(&mut n.children, at_line_start, is_paragraph_start),
            Node::Link(n) => prepare_inline(&mut n.children, at_line_start, is_paragraph_start),
            _ => {
                *at_line_start = false;
                *is_paragraph_start = false;
            }
        }
    }
}

fn escape_list_marker(line: &str) -> std::borrow::Cow<'_, str> {
    let mut chars = line.chars();
    match chars.next() {
        Some('-' | '*' | '+') if chars.next() == Some(' ') => format!("\\{line}").into(),
        Some(character) if character.is_ascii_digit() => {
            let digits_end = line
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .count();
            let rest = &line[digits_end..];
            if rest.starts_with(". ") || rest.starts_with(") ") {
                format!("{}\\{rest}", &line[..digits_end]).into()
            } else {
                line.into()
            }
        }
        _ => line.into(),
    }
}
