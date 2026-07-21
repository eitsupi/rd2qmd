//! Flattening arbitrary block content down to inline nodes safe for a GFM
//! pipe-table cell.

use rd_ast::RdNode;
use rd2qmd_mdast::{Html, Node};

use super::markdown_text::node_to_markdown_string;
use super::{BlockConversionContext, convert_block_content, replace_line_endings_with_space};

/// Flatten block content to inline nodes for GFM table cells.
/// Uses `<br>` for paragraph breaks and flattens lists with bullet markers.
pub(super) fn flatten_for_table_cell(
    content: &[RdNode],
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    let block_nodes = convert_block_content(content, context);
    let mut result = Vec::new();

    for (i, node) in block_nodes.iter().enumerate() {
        if i > 0 && !result.is_empty() {
            result.push(Node::Html(Html {
                value: " <br>".to_owned(),
            }));
        }
        result.extend(flatten_block_node_for_table_cell(node));
    }

    result
}

/// Flatten one block node -- either a top-level node produced by
/// `convert_block_content`, or a child of a `ListItem` produced by the same
/// function's list-handling arm -- into inline nodes safe to embed directly
/// as a GFM pipe-table cell's children. Shared between the top level of
/// [`flatten_for_table_cell`] and its own `Node::List` arm so a list item's
/// non-paragraph child (e.g. a nested `\describe{}` or `\preformatted{}`)
/// gets exactly the same treatment as a top-level block of that shape.
fn flatten_block_node_for_table_cell(node: &Node) -> Vec<Node> {
    let mut result = Vec::new();
    match node {
        Node::Paragraph(paragraph) => {
            extend_table_cell_inline(&mut result, &paragraph.children);
        }
        Node::List(list) => {
            for (j, item) in list.children.iter().enumerate() {
                if j > 0 {
                    result.push(Node::Html(Html {
                        value: " <br>".to_owned(),
                    }));
                }
                if let Node::ListItem(item) = item {
                    let marker = if list.ordered {
                        format!("{}. ", j + 1)
                    } else {
                        "- ".to_owned()
                    };
                    result.push(Node::text(marker));
                    for (k, child) in item.children.iter().enumerate() {
                        if k > 0 {
                            result.push(Node::Html(Html {
                                value: " <br>".to_owned(),
                            }));
                        }
                        result.extend(flatten_block_node_for_table_cell(child));
                    }
                }
            }
        }
        Node::Code(code) => {
            // `\preformatted{}` content: pipe-table cells are single-line, so
            // render as an inline code span with line endings flattened.
            // This is raw content that was never touched by
            // `sanitize_table_cell_inline_nodes`, so any backslash already
            // present is genuine data (e.g. a literal `\|` in a regex or
            // path), not pre-existing table escaping. Escape blindly:
            // Pandoc's pipe-table row splitter consumes exactly one escaping
            // backslash per `\|` before the code span is even parsed, so a
            // *single* pre-existing backslash would be silently swallowed,
            // dropping it from the displayed code. Adding one more backslash
            // (`\|` -> `\\|`) makes the row splitter treat the pipe as
            // literal *and* leaves one literal backslash behind in the
            // rendered code -- verified empirically against Pandoc 3.7.
            let value = replace_line_endings_with_space(&code.value).replace('|', "\\|");
            result.push(Node::inline_code(value));
        }
        Node::Table(_) => {
            // Structurally complex block content that cannot become a
            // simple inline node. Serialize through the real writer, then
            // flatten to pipe-table-cell-safe text: this degrades
            // gracefully (content is present, if not beautifully formatted)
            // rather than vanishing. Unlike `Code`/`DefinitionList`/`Math`
            // below, a nested table's cell content has *already* been
            // escaped once by `sanitize_table_cell_inline_nodes` inside
            // `convert_tabular` -- so this arm must use the parity-aware
            // `escape_unescaped_pipes` (not a blind replace) to add
            // protective escaping only to the table's own fresh delimiter
            // pipes, without doubling the pre-existing escape and flipping
            // it back to "unescaped" from the outer row-splitter's view.
            let markdown = node_to_markdown_string(node);
            let flattened = escape_unescaped_pipes(&replace_line_endings_with_space(&markdown));
            result.push(Node::Html(Html { value: flattened }));
        }
        Node::DefinitionList(list) => {
            // Unlike `Math`/`Heading` below, a `\describe{}` item's body is
            // produced by `convert_block_content` (see the `RdTag::Describe`
            // arm of `convert_block`), so it can itself contain a nested
            // `Table` whose cells were *already* escaped once by
            // `sanitize_table_cell_inline_nodes` inside `convert_tabular`.
            // Serializing the whole subtree to one string and then
            // blind-escaping it (the old approach) would double-escape
            // those pre-escaped pipes. Recurse per child instead -- exactly
            // like the `List` arm above -- so each nested node picks its
            // own correct escaping rule (a nested `Table` gets parity-aware
            // treatment via this same match, raw leaves still get blind
            // escaping).
            // Pair each term with its following description(s) -- mirroring
            // `write_definition_list`'s own term/description grouping --
            // rather than inserting a `<br>` between every adjacent child,
            // which would split a "term: description" pair across a break.
            let mut i = 0;
            let mut first_entry = true;
            while i < list.children.len() {
                let Node::DefinitionTerm(term) = &list.children[i] else {
                    i += 1;
                    continue;
                };
                if !first_entry {
                    result.push(Node::Html(Html {
                        value: " <br>".to_owned(),
                    }));
                }
                first_entry = false;
                extend_table_cell_inline(&mut result, &term.children);
                result.push(Node::text(": "));
                i += 1;

                let mut first_desc_child = true;
                while let Some(Node::DefinitionDescription(description)) = list.children.get(i) {
                    for desc_child in &description.children {
                        if !first_desc_child {
                            result.push(Node::Html(Html {
                                value: " <br>".to_owned(),
                            }));
                        }
                        first_desc_child = false;
                        result.extend(flatten_block_node_for_table_cell(desc_child));
                    }
                    i += 1;
                }
            }
        }
        Node::Math(_) | Node::Heading(_) => {
            // Same graceful-degradation approach as `Table` above, but
            // these two variants are never pre-escaped by
            // `sanitize_table_cell_inline_nodes` before reaching here (a
            // `Heading` here only ever holds a nested `\section`'s inline
            // title text, never a nested block -- see
            // `convert_section_like_block`), so any backslash already
            // present is genuine content (e.g. semantic TeX in a `\deqn`).
            // Escape blindly, for the same reason as the `Code` arm above:
            // this preserves a literal backslash in the rendered output
            // instead of letting the outer pipe-table row splitter silently
            // consume it.
            let markdown = node_to_markdown_string(node);
            let flattened = replace_line_endings_with_space(&markdown).replace('|', "\\|");
            result.push(Node::Html(Html { value: flattened }));
        }
        // `convert_block_content` (whose output, directly or via a
        // `ListItem`'s children, is the only input this helper ever sees)
        // cannot produce any of these variants at this position: its
        // producers are limited to `convert_paragraph` (Paragraph),
        // `convert_block`'s match arms (List, DefinitionList, Code, Math,
        // Table, and Heading via nested `\section`/`\subsection`), and the
        // roxygen code-block path (Code). Keep this arm exhaustive (no
        // wildcard) so a newly added `Node` variant fails to compile here
        // instead of silently vanishing.
        Node::ThematicBreak
        | Node::Blockquote(_)
        | Node::ListItem(_)
        | Node::TableRow(_)
        | Node::TableCell(_)
        | Node::DefinitionTerm(_)
        | Node::DefinitionDescription(_)
        | Node::Text(_)
        | Node::Emphasis(_)
        | Node::Strong(_)
        | Node::InlineCode(_)
        | Node::Break
        | Node::Link(_)
        | Node::Image(_)
        | Node::InlineMath(_)
        | Node::Html(_) => {
            debug_assert!(
                false,
                "convert_block_content cannot produce {node:?} as a top-level block node or list-item child"
            );
        }
    }
    result
}

fn extend_table_cell_inline(result: &mut Vec<Node>, nodes: &[Node]) {
    result.extend(sanitize_table_cell_inline_nodes(nodes));
}

/// Normalize a sequence of inline nodes for embedding directly inside a
/// `TableCell`'s `children`. Unlike [`sanitize_table_cell_inline_node`],
/// this flattens any `Node::Paragraph` (produced when a multi-node
/// conditional branch collapses, see `collapse_inline_nodes`) by splicing
/// its sanitized children in place, since a `Paragraph` is a block node the
/// real writer cannot safely nest inside a table cell.
pub(super) fn sanitize_table_cell_inline_nodes(nodes: &[Node]) -> Vec<Node> {
    let mut result = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            Node::Paragraph(paragraph) => {
                result.extend(sanitize_table_cell_inline_nodes(&paragraph.children));
            }
            _ => result.push(sanitize_table_cell_inline_node(node)),
        }
    }
    result
}

pub(super) fn sanitize_table_cell_inline_node(node: &Node) -> Node {
    let mut node = node.clone();
    match &mut node {
        Node::Text(text) => {
            text.value = replace_line_endings_with_space(&text.value).replace('|', "\\|");
        }
        Node::InlineCode(code) => {
            code.value = replace_line_endings_with_space(&code.value).replace('|', "\\|");
        }
        // Accepted limitation: flattening LaTeX line endings is not TeX-comment-aware.
        // An unescaped `%` may therefore comment out content that originally followed
        // on a later line; `\%` is a literal percent. Preserving TeX tokenization while
        // producing a single-line pipe-table cell would require TeX-aware parsing.
        Node::InlineMath(math) => {
            math.value = replace_line_endings_with_space(&math.value).replace('|', "\\|");
        }
        Node::Math(math) => {
            return Node::InlineMath(rd2qmd_mdast::InlineMath {
                value: replace_line_endings_with_space(&math.value).replace('|', "\\|"),
            });
        }
        Node::Image(image) => {
            image.url = replace_line_endings_with_space(&image.url).replace('|', "\\|");
            image.alt = replace_line_endings_with_space(&image.alt).replace('|', "\\|");
            if let Some(title) = &mut image.title {
                *title = replace_line_endings_with_space(title).replace('|', "\\|");
            }
        }
        Node::Html(html) => {
            html.value = replace_line_endings_with_space(&html.value).replace('|', "\\|");
        }
        Node::Break => {
            return Node::Html(Html {
                value: "<br>".to_owned(),
            });
        }
        Node::Emphasis(emphasis) => {
            emphasis.children = sanitize_table_cell_inline_nodes(&emphasis.children);
        }
        Node::Strong(strong) => {
            strong.children = sanitize_table_cell_inline_nodes(&strong.children);
        }
        Node::Link(link) => {
            link.url = replace_line_endings_with_space(&link.url).replace('|', "\\|");
            if let Some(title) = &mut link.title {
                *title = replace_line_endings_with_space(title).replace('|', "\\|");
            }
            link.children = sanitize_table_cell_inline_nodes(&link.children);
        }
        _ => {}
    }
    node
}

/// Escape every `|` in `value` that is not already escaped by an
/// immediately preceding odd-length run of backslashes. Used when
/// flattening already-serialized Markdown (which may already contain
/// correctly-escaped pipes from its own nested table-cell construction)
/// into an outer pipe-table cell, so a pipe that's already escaped once
/// doesn't get escaped a second time and re-exposed as an unescaped `|`
/// once the outer escaping backslash is itself consumed by CommonMark's
/// backslash-escape processing.
fn escape_unescaped_pipes(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut backslash_run = 0usize;
    for ch in value.chars() {
        if ch == '|' {
            if backslash_run.is_multiple_of(2) {
                result.push('\\');
            }
            result.push('|');
            backslash_run = 0;
        } else {
            backslash_run = if ch == '\\' { backslash_run + 1 } else { 0 };
            result.push(ch);
        }
    }
    result
}
