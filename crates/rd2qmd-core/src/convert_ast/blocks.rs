//! General block-content assembly for the rd_ast conversion migration.

use rd_ast::{RdArgument, RdListItem, RdListKind, RdNode, RdPath, RdTag};
use rd2qmd_mdast::{Align, Html, Node, Root, WriterOptions, mdast_to_qmd};
use tabled::settings::Style;
use tabled::settings::style::HorizontalLine;

use super::{
    inline::{self, InlineConversionContext},
    leaf_text::flatten_verbatim_leaves,
    traversal::{BlockContentItem, ParagraphItem, scan_block_content},
};

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

/// Flatten block content to inline nodes for GFM table cells.
/// Uses `<br>` for paragraph breaks and flattens lists with bullet markers.
fn flatten_for_table_cell(content: &[RdNode], context: &BlockConversionContext<'_>) -> Vec<Node> {
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
fn sanitize_table_cell_inline_nodes(nodes: &[Node]) -> Vec<Node> {
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

fn sanitize_table_cell_inline_node(node: &Node) -> Node {
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

/// Convert Rd content to a standalone Markdown string for a grid-table cell.
///
/// Grid tables are built from raw strings by `tabled`, whereas the main mdast
/// writer owns one global output buffer and tracks whole-document line state.
/// The dedicated serializers below therefore preserve the legacy subtree path
/// until the writer can directly serialize isolated AST fragments.
fn convert_to_markdown_text(content: &[RdNode], context: &BlockConversionContext<'_>) -> String {
    nodes_to_markdown(&convert_block_content(content, context))
}

fn nodes_to_markdown(nodes: &[Node]) -> String {
    let mut result = String::new();

    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            result.push_str("\n\n");
        }

        match node {
            Node::Paragraph(paragraph) => {
                let text = inline_nodes_to_markdown(&paragraph.children);
                result.push_str(&indent_cell_continuation(&text, ""));
            }
            Node::List(list) => {
                if i > 0 && !result.ends_with("\n\n") {
                    result.push('\n');
                }
                for (j, item) in list.children.iter().enumerate() {
                    if j > 0 {
                        result.push('\n');
                    }
                    if let Node::ListItem(item) = item {
                        let marker = if list.ordered {
                            format!("{}. ", j + 1)
                        } else {
                            "- ".to_owned()
                        };
                        let continuation = " ".repeat(marker.len());
                        result.push_str(&marker);
                        for (k, child) in item.children.iter().enumerate() {
                            if k > 0 {
                                result.push_str("\n\n");
                                result.push_str(&continuation);
                            }
                            match child {
                                Node::Paragraph(paragraph) => {
                                    let text = inline_nodes_to_markdown(&paragraph.children);
                                    result
                                        .push_str(&indent_cell_continuation(&text, &continuation));
                                }
                                other => {
                                    let text = node_to_markdown_string(other);
                                    for (i, line) in text.split('\n').enumerate() {
                                        if i > 0 {
                                            result.push('\n');
                                            if !line.is_empty() {
                                                result.push_str(&continuation);
                                            }
                                        }
                                        result.push_str(line);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Node::Code(code) => {
                let fence = code_fence(&code.value);
                result.push_str(&fence);
                if let Some(lang) = &code.lang {
                    result.push_str(lang);
                }
                result.push('\n');
                result.push_str(&code.value);
                result.push('\n');
                result.push_str(&fence);
            }
            Node::Math(_) | Node::DefinitionList(_) | Node::Table(_) | Node::Heading(_) => {
                result.push_str(&node_to_markdown_string(node));
            }
            // `convert_block_content` (this function's only caller-of-callers,
            // via `convert_to_markdown_text`) cannot produce any of these
            // variants as a top-level block node or list-item child: its
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
                    "convert_block_content cannot produce {node:?} as a top-level block node"
                );
            }
        }
    }

    result
}

fn inline_nodes_to_markdown(nodes: &[Node]) -> String {
    let mut result = String::new();

    for node in nodes {
        match node {
            Node::Text(text) => result.push_str(&text.value),
            Node::InlineCode(code) => result.push_str(&rd2qmd_mdast::format_inline_code(
                &code.value,
                result.ends_with('`'),
            )),
            Node::Emphasis(emphasis) => {
                result.push('*');
                result.push_str(&inline_nodes_to_markdown(&emphasis.children));
                result.push('*');
            }
            Node::Strong(strong) => {
                result.push_str("**");
                result.push_str(&inline_nodes_to_markdown(&strong.children));
                result.push_str("**");
            }
            Node::Link(link) => {
                result.push('[');
                result.push_str(&inline_nodes_to_markdown(&link.children));
                result.push_str("](");
                result.push_str(&rd2qmd_mdast::format_link_destination(&link.url));
                if let Some(title) = &link.title {
                    result.push_str(" \"");
                    result.push_str(&rd2qmd_mdast::escape_link_title(title));
                    result.push('"');
                }
                result.push(')');
            }
            Node::InlineMath(math) => {
                result.push('$');
                result.push_str(&math.value);
                result.push('$');
            }
            Node::Image(image) => {
                result.push_str("![");
                result.push_str(&image.alt);
                result.push_str("](");
                result.push_str(&rd2qmd_mdast::format_link_destination(&image.url));
                if let Some(title) = &image.title {
                    result.push_str(" \"");
                    result.push_str(&rd2qmd_mdast::escape_link_title(title));
                    result.push('"');
                }
                result.push(')');
            }
            Node::Break => result.push_str("  \n"),
            Node::Html(html) => result.push_str(&html.value),
            _ => {
                if let Some(text) = node_to_text(node) {
                    result.push_str(&text);
                }
            }
        }
    }

    result
}

fn node_to_text(node: &Node) -> Option<String> {
    match node {
        Node::Text(text) => Some(text.value.clone()),
        Node::InlineCode(code) => Some(format!("`{}`", code.value)),
        Node::Paragraph(paragraph) => Some(inline_nodes_to_markdown(&paragraph.children)),
        _ => None,
    }
}

fn render_list_table_cell(nodes: &[Node]) -> String {
    render_block_content(nodes, 4)
}

fn render_block_content(nodes: &[Node], indent: u8) -> String {
    let indent = " ".repeat(indent as usize);
    let indent = indent.as_str();
    let mut result = String::new();
    let mut first_block = true;

    for node in nodes {
        match node {
            Node::Paragraph(paragraph) => {
                let text = inline_nodes_to_markdown(&paragraph.children);
                let text = text.trim_start();
                if text.is_empty() {
                    continue;
                }
                let indented = indent_cell_continuation(text, indent);
                if first_block {
                    result.push_str(&indented);
                } else {
                    result.push_str("\n\n");
                    result.push_str(indent);
                    result.push_str(&indented);
                }
                first_block = false;
            }
            Node::List(list) => {
                let mut any_item = false;
                for (j, list_item) in list.children.iter().enumerate() {
                    if let Node::ListItem(item) = list_item {
                        let marker = if list.ordered {
                            format!("{}. ", j + 1)
                        } else {
                            "- ".to_owned()
                        };
                        let outer = if !first_block || j > 0 { indent } else { "" };
                        let continuation = format!("{}{}", indent, " ".repeat(marker.len()));
                        let (first_child_text, first_was_paragraph) = item
                            .children
                            .first()
                            .map(|child| {
                                if let Node::Paragraph(paragraph) = child {
                                    (
                                        indent_cell_continuation(
                                            &inline_nodes_to_markdown(&paragraph.children),
                                            &continuation,
                                        ),
                                        true,
                                    )
                                } else {
                                    (String::new(), false)
                                }
                            })
                            .unwrap_or((String::new(), false));

                        if !any_item {
                            if !first_block {
                                result.push_str("\n\n");
                            }
                        } else {
                            result.push('\n');
                        }
                        result.push_str(outer);
                        result.push_str(&marker);
                        result.push_str(&first_child_text);
                        any_item = true;

                        let skip_n = usize::from(first_was_paragraph);
                        for child in item.children.iter().skip(skip_n) {
                            let text = node_to_markdown_string(child);
                            if text.is_empty() {
                                continue;
                            }
                            result.push_str("\n\n");
                            result.push_str(&continuation);
                            for (i, line) in text.split('\n').enumerate() {
                                if i > 0 {
                                    result.push('\n');
                                    if !line.is_empty() {
                                        result.push_str(&continuation);
                                    }
                                }
                                result.push_str(line);
                            }
                        }
                    }
                }
                if any_item {
                    first_block = false;
                }
            }
            Node::Code(code) => {
                let lang = code.lang.as_deref().unwrap_or("");
                let fence = code_fence(&code.value);
                result.push_str("\n\n");
                result.push_str(indent);
                result.push_str(&fence);
                result.push_str(lang);
                let lines: Vec<_> = code.value.split('\n').collect();
                let code_lines = if lines.last() == Some(&"") {
                    &lines[..lines.len() - 1]
                } else {
                    &lines[..]
                };
                for line in code_lines {
                    result.push('\n');
                    if !line.is_empty() {
                        result.push_str(indent);
                        result.push_str(line);
                    }
                }
                result.push('\n');
                result.push_str(indent);
                result.push_str(&fence);
                first_block = false;
            }
            Node::Math(_) | Node::DefinitionList(_) | Node::Table(_) => {
                let text = node_to_markdown_string(node);
                if text.is_empty() {
                    continue;
                }
                result.push_str("\n\n");
                result.push_str(indent);
                for (i, line) in text.split('\n').enumerate() {
                    if i > 0 {
                        result.push('\n');
                        if !line.is_empty() {
                            result.push_str(indent);
                        }
                    }
                    result.push_str(line);
                }
                first_block = false;
            }
            _ => {
                if let Some(text) = node_to_text(node) {
                    let text = text.trim_start();
                    if text.is_empty() {
                        continue;
                    }
                    if first_block {
                        result.push_str(text);
                    } else {
                        result.push_str("\n\n");
                        result.push_str(indent);
                        result.push_str(text);
                    }
                    first_block = false;
                }
            }
        }
    }

    result
}

/// Serialize one mdast node through the real writer.
fn node_to_markdown_string(node: &Node) -> String {
    let root = Root::new(vec![node.clone()]);
    let options = WriterOptions {
        frontmatter: None,
        // The migration context has no writer-format option yet. Preserve the
        // legacy call site's default until the document converter is wired.
        quarto_code_blocks: true,
    };
    mdast_to_qmd(&root, &options).trim().to_owned()
}

/// Return a backtick fence long enough to wrap `code`.
fn code_fence(code: &str) -> String {
    let max_run = code
        .split('\n')
        .filter_map(|line| {
            let line = line.trim_start();
            line.starts_with('`').then(|| {
                line.chars()
                    .take_while(|character| *character == '`')
                    .count()
            })
        })
        .max()
        .unwrap_or(0);
    "`".repeat(max_run.max(2) + 1)
}

/// Prefix every continuation line after the first newline with `prefix`.
fn indent_cell_continuation(text: &str, prefix: &str) -> String {
    let mut result = String::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            result.push('\n');
            let line = line.trim_start();
            if !line.is_empty() {
                result.push_str(prefix);
                result.push_str(&escape_md_list_marker(line));
            }
        } else {
            result.push_str(line);
        }
    }
    result
}

fn escape_md_list_marker(line: &str) -> std::borrow::Cow<'_, str> {
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

fn convert_paragraph(
    items: Vec<ParagraphItem<'_>>,
    context: &BlockConversionContext<'_>,
) -> Option<Node> {
    let children: Vec<_> = items
        .into_iter()
        .flat_map(|item| match item {
            ParagraphItem::Text(text) => vec![inline::convert_text(text)],
            ParagraphItem::Node(node) => {
                inline::convert_inline_nodes(std::slice::from_ref(node), &context.inline)
            }
        })
        .collect();

    (!children.is_empty()).then(|| Node::paragraph(children))
}

fn convert_block(node: &RdNode, context: &BlockConversionContext<'_>) -> Vec<Node> {
    let Some(tagged) = node.as_tagged() else {
        return Vec::new();
    };
    let base_path = RdPath::new(Vec::new());

    match tagged.tag() {
        RdTag::Itemize | RdTag::Enumerate => {
            let Some(list) = tagged.inspect_list(&base_path).ok() else {
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
                    RdListItem::Delimited(item) => {
                        Some(Node::list_item(convert_block_content(item.body(), context)))
                    }
                    _ => None,
                })
                .collect();
            vec![Node::list(ordered, items)]
        }
        RdTag::Describe => {
            let Some(list) = tagged.inspect_list(&base_path).ok() else {
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
                children.push(Node::definition_term(inline::convert_inline_nodes(
                    item.label(),
                    &context.inline,
                )));
                // Unlike the legacy inline-only description, preserve arbitrary
                // block children such as multiple paragraphs and nested lists.
                children.push(Node::definition_description(convert_block_content(
                    item.body(),
                    context,
                )));
            }
            vec![Node::definition_list(children)]
        }
        RdTag::Preformatted => vec![Node::code(None, recover_verbatim(tagged.children()))],
        RdTag::Deqn => {
            let Some(equation) = tagged.inspect_equation(&base_path).ok() else {
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
            vec![Node::math(equation_text(equation.latex(), &context.inline))]
        }
        RdTag::Tabular => convert_tabular(tagged, context).into_iter().collect(),
        RdTag::Section | RdTag::Subsection => convert_section_like_block(tagged, context),
        _ => Vec::new(),
    }
}

fn convert_tabular(
    tagged: &rd_ast::RdTagged,
    context: &BlockConversionContext<'_>,
) -> Option<Node> {
    let base_path = RdPath::new(Vec::new());
    let table = tagged.inspect_tabular(&base_path).ok()?;
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
            let cells =
                row.cells()
                    .iter()
                    .map(|cell| {
                        let children = sanitize_table_cell_inline_nodes(
                            &inline::convert_inline_nodes(cell.nodes(), &context.inline),
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
    tagged: &rd_ast::RdTagged,
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    if tagged.option().is_some() {
        return Vec::new();
    }
    let [title, body] = tagged.children() else {
        return Vec::new();
    };
    let (Some(title), Some(body)) = (title.as_group(), body.as_group()) else {
        return Vec::new();
    };
    let depth = context.enclosing_heading_depth.saturating_add(1).min(6);
    let mut nodes = vec![Node::heading(
        depth,
        inline::convert_inline_nodes(title.children(), &context.inline),
    )];
    let child_context = BlockConversionContext {
        enclosing_heading_depth: depth,
        ..*context
    };
    nodes.extend(convert_block_content(body.children(), &child_context));
    nodes
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

fn equation_text(nodes: &[RdNode], context: &InlineConversionContext<'_>) -> String {
    inline::extract_plain_text(&inline::convert_inline_nodes(nodes, context))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rd_ast::{RdDocument, RdNode, RdTag};
    use rd2qmd_mdast::{Node, Root, WriterOptions, mdast_to_qmd};

    use super::{
        BlockConversionContext, convert_arguments, convert_block_content, convert_custom_section,
        convert_to_markdown_text, inline_nodes_to_markdown, sanitize_table_cell_inline_node,
    };
    use crate::ArgumentsFormat;
    use crate::convert_ast::document::build_custom_sections;
    use crate::convert_ast::inline::{InlineConversionContext, LinkResolutionContext};

    fn context(prefer_ascii_math: bool) -> BlockConversionContext<'static> {
        BlockConversionContext {
            inline: InlineConversionContext::default(),
            prefer_ascii_math,
            enclosing_heading_depth: 2,
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

    fn tagged_with_option(tag: RdTag, option: &str, children: Vec<RdNode>) -> RdNode {
        RdNode::tagged(tag, Some(vec![text(option)]), children)
    }

    fn section_like(tag: RdTag, title: &str, body: Vec<RdNode>) -> RdNode {
        tagged(tag, vec![group(vec![text(title)]), group(body)])
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

    fn argument_document() -> RdDocument {
        let arguments = vec![
            described_item(
                vec![text(" alpha ")],
                vec![text("First paragraph\n\nSecond paragraph")],
            ),
            described_item(
                vec![tagged(RdTag::Code, vec![text("choice")])],
                vec![
                    text("Choices:\n\n"),
                    delimited_list(RdTag::Itemize, vec![vec![text("one")], vec![text("two")]]),
                ],
            ),
        ];
        RdDocument::new(vec![tagged(RdTag::Arguments, arguments)])
    }

    fn argument_document_with_description(description: Vec<RdNode>) -> RdDocument {
        RdDocument::new(vec![tagged(
            RdTag::Arguments,
            vec![described_item(vec![text("value")], description)],
        )])
    }

    fn html_value(nodes: &[Node]) -> &str {
        let [Node::Html(html)] = nodes else {
            panic!("expected one raw output node")
        };
        &html.value
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
    fn block_equation_preserves_multiline_ascii_layout() {
        let matrix = equation("matrix", Some("[ 1 2 ]\n[ 3 4 ]"));

        assert_eq!(
            convert_block_content(&[matrix], &context(true)),
            vec![Node::code(None, "[ 1 2 ]\n[ 3 4 ]")]
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

    #[test]
    fn converts_tabular_alignment_rows_and_sanitized_cells() {
        let table = tagged(
            RdTag::Tabular,
            vec![
                group(vec![text("lcr")]),
                group(vec![
                    text("left | value"),
                    tagged(RdTag::Tab, vec![]),
                    tagged(RdTag::Strong, vec![text("center")]),
                    tagged(RdTag::Tab, vec![]),
                    tagged(RdTag::Code, vec![text("right|code")]),
                    tagged(RdTag::Cr, vec![]),
                    text("second row"),
                ]),
            ],
        );

        let converted = convert_block_content(&[table], &context(false));
        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        assert_eq!(
            table.align,
            [
                Some(rd2qmd_mdast::Align::Left),
                Some(rd2qmd_mdast::Align::Center),
                Some(rd2qmd_mdast::Align::Right),
            ]
        );
        assert_eq!(table.children.len(), 2);
        let Node::TableRow(first_row) = &table.children[0] else {
            panic!("expected first table row")
        };
        let cell_text: Vec<_> = first_row
            .children
            .iter()
            .map(|cell| match cell {
                Node::TableCell(cell) => inline_nodes_to_markdown(&cell.children),
                _ => panic!("expected table cell"),
            })
            .collect();
        assert_eq!(
            cell_text,
            ["left \\| value", "**center**", "`right\\|code`"]
        );
    }

    #[test]
    fn converts_tabular_block_math_to_pipe_escaped_inline_math() {
        let table = tagged(
            RdTag::Tabular,
            vec![group(vec![text("l")]), group(vec![equation("x | y", None)])],
        );

        let converted = convert_block_content(&[table], &context(false));
        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        let [Node::TableRow(row)] = table.children.as_slice() else {
            panic!("expected one table row")
        };
        let [Node::TableCell(cell)] = row.children.as_slice() else {
            panic!("expected one table cell")
        };
        assert_eq!(cell.children, [Node::inline_math("x \\| y")]);
    }

    #[test]
    fn converts_tabular_multiline_block_math_to_single_line_inline_math() {
        let table = tagged(
            RdTag::Tabular,
            vec![
                group(vec![text("l")]),
                group(vec![equation("x |\ny", None)]),
            ],
        );

        let converted = convert_block_content(&[table], &context(false));
        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        let [Node::TableRow(row)] = table.children.as_slice() else {
            panic!("expected one table row")
        };
        let [Node::TableCell(cell)] = row.children.as_slice() else {
            panic!("expected one table cell")
        };
        assert_eq!(cell.children, [Node::inline_math("x \\| y")]);
        let [Node::InlineMath(math)] = cell.children.as_slice() else {
            panic!("expected inline math")
        };
        assert!(!math.value.contains('\n'));
    }

    #[test]
    fn converts_tabular_multiline_ascii_equation_to_single_line_inline_code() {
        let table = tagged(
            RdTag::Tabular,
            vec![
                group(vec![text("l")]),
                group(vec![equation("x^2 + y^2", Some("x^2 +\n y^2"))]),
            ],
        );

        let mut table_context = context(true);
        table_context.inline.prefer_ascii_math = true;
        let converted = convert_block_content(&[table], &table_context);
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );
        let row = markdown
            .lines()
            .find(|line| line.contains("x^2"))
            .expect("expected equation table row");
        assert_eq!(row, "| `x^2 +  y^2` |");
    }

    #[test]
    fn pipe_table_sanitizer_replaces_only_line_endings() {
        let sanitized = sanitize_table_cell_inline_node(&Node::inline_code("a  b\tc\r\nd\re\n f"));
        assert!(matches!(
            sanitized,
            Node::InlineCode(code) if code.value == "a  b\tc d e  f"
        ));
    }

    #[test]
    fn converts_multiline_out_in_tabular_cell_to_one_pipe_table_row() {
        let table = tagged(
            RdTag::Tabular,
            vec![
                group(vec![text("l")]),
                group(vec![tagged(RdTag::Out, vec![text("first\nsecond")])]),
            ],
        );

        let converted = convert_block_content(&[table], &context(false));
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );
        let rows: Vec<_> = markdown
            .lines()
            .filter(|line| line.starts_with('|'))
            .collect();
        assert_eq!(rows, ["| first second |", "|:---|"]);
    }

    #[test]
    fn pipe_table_sanitizer_replaces_link_title_line_endings() {
        let mut url = String::from(r"url");
        url.push('\n');
        url.push_str(r"value");
        let mut title = String::from(r"title");
        title.push('\r');
        title.push('\n');
        title.push_str(r"value");
        let sanitized = sanitize_table_cell_inline_node(&Node::link_with_title(
            url,
            title,
            vec![Node::text(r"link")],
        ));
        let markdown = mdast_to_qmd(
            &Root::new(vec![Node::paragraph(vec![sanitized])]),
            &WriterOptions::default(),
        );

        let mut expected = String::from(r#"[link](<url value> "title value")"#);
        expected.push('\n');
        assert_eq!(markdown, expected);
    }

    #[test]
    fn pipe_table_sanitizer_replaces_image_field_line_endings() {
        let mut url = String::from(r"url");
        url.push('\r');
        url.push_str(r"value");
        let mut alt = String::from(r"alt");
        alt.push('\n');
        alt.push_str(r"value");
        let mut title = String::from(r"title");
        title.push('\r');
        title.push('\n');
        title.push_str(r"value");
        let sanitized = sanitize_table_cell_inline_node(&Node::image_with_title(url, alt, title));
        let markdown = mdast_to_qmd(
            &Root::new(vec![Node::paragraph(vec![sanitized])]),
            &WriterOptions::default(),
        );

        let mut expected = String::from(r#"![alt value](<url value> "title value")"#);
        expected.push('\n');
        assert_eq!(markdown, expected);
    }

    #[test]
    fn legacy_table_cell_serializer_formats_link_and_image_destinations() {
        let markdown = inline_nodes_to_markdown(&[
            Node::link_with_title(
                r"link url",
                r#"link "title" \ docs"#,
                vec![Node::text(r"link")],
            ),
            Node::text(r" "),
            Node::image_with_title(r"image url", r"alt", r#"image "title" \ docs"#),
        ]);

        assert_eq!(
            markdown,
            r#"[link](<link url> "link \"title\" \\ docs") ![alt](<image url> "image \"title\" \\ docs")"#
        );
    }

    #[test]
    fn converts_section_like_block_outside_section_tree() {
        let subsection = section_like(
            RdTag::Subsection,
            "Orphan subsection",
            vec![text("orphan body")],
        );

        let converted = convert_block_content(&[subsection], &context(false));
        assert!(matches!(
            converted.as_slice(),
            [Node::Heading(heading), Node::Paragraph(paragraph)]
                if heading.depth == 3
                    && inline_nodes_to_markdown(&heading.children) == "Orphan subsection"
                    && inline_nodes_to_markdown(&paragraph.children) == "orphan body"
        ));
    }

    #[test]
    fn custom_section_tree_preserves_content_around_nested_subsections() {
        let document = RdDocument::new(vec![section_like(
            RdTag::Section,
            "Parent",
            vec![
                text("intro"),
                section_like(RdTag::Subsection, "First", vec![text("first body")]),
                text("between"),
                section_like(RdTag::Subsection, "Second", vec![text("second body")]),
                text("after"),
            ],
        )]);
        let sections = build_custom_sections(&document);

        let converted = convert_custom_section(&sections[0], &context(false));
        let summary: Vec<_> = converted
            .iter()
            .map(|node| match node {
                Node::Heading(heading) => format!(
                    "h{}:{}",
                    heading.depth,
                    inline_nodes_to_markdown(&heading.children)
                ),
                Node::Paragraph(paragraph) => {
                    format!("p:{}", inline_nodes_to_markdown(&paragraph.children))
                }
                _ => panic!("unexpected custom-section node: {node:?}"),
            })
            .collect();
        assert_eq!(
            summary,
            [
                "h2:Parent",
                "p:intro",
                "h3:First",
                "p:first body",
                "p:between",
                "h3:Second",
                "p:second body",
                "p:after",
            ]
        );
    }

    #[test]
    fn converts_arguments_to_pipe_table_and_flattens_lists_with_breaks() {
        let document = argument_document();
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));

        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        assert_eq!(table.align, [Some(rd2qmd_mdast::Align::Left); 2]);
        assert_eq!(table.children.len(), 3);

        let Node::TableRow(first_argument) = &table.children[1] else {
            panic!("expected first argument row")
        };
        let Node::TableCell(first_description) = &first_argument.children[1] else {
            panic!("expected first description cell")
        };
        assert_eq!(
            inline_nodes_to_markdown(&first_description.children),
            "First paragraph <br>Second paragraph"
        );

        let Node::TableRow(second_argument) = &table.children[2] else {
            panic!("expected second argument row")
        };
        let Node::TableCell(second_description) = &second_argument.children[1] else {
            panic!("expected second description cell")
        };
        assert_eq!(
            inline_nodes_to_markdown(&second_description.children),
            "Choices: <br>- one <br>- two"
        );
    }

    #[test]
    fn pipe_table_replaces_inline_breaks_with_html_breaks() {
        let document = argument_document_with_description(vec![
            text("before"),
            tagged(RdTag::Cr, vec![]),
            text("after"),
        ]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));

        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        let Node::TableRow(row) = &table.children[1] else {
            panic!("expected argument row")
        };
        let Node::TableCell(description) = &row.children[1] else {
            panic!("expected description cell")
        };

        assert_eq!(
            inline_nodes_to_markdown(&description.children),
            "before<br>after"
        );
        assert!(
            description
                .children
                .iter()
                .any(|node| matches!(node, Node::Html(html) if html.value == "<br>"))
        );
        assert!(
            !description
                .children
                .iter()
                .any(|node| matches!(node, Node::Break))
        );
    }

    #[test]
    fn pipe_table_escapes_literal_pipes_in_text() {
        let document = argument_document_with_description(vec![text("left | right")]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));

        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        let Node::TableRow(row) = &table.children[1] else {
            panic!("expected argument row")
        };
        let Node::TableCell(description) = &row.children[1] else {
            panic!("expected description cell")
        };

        assert_eq!(
            inline_nodes_to_markdown(&description.children),
            "left \\| right"
        );
        assert!(description.children.iter().all(|node| {
            !matches!(node, Node::Text(text) if text.value.contains('|') && !text.value.contains("\\|"))
        }));
    }

    #[test]
    fn pipe_table_escapes_literal_pipes_in_inline_code() {
        let document =
            argument_document_with_description(vec![tagged(RdTag::Code, vec![text("a|b")])]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));

        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        let Node::TableRow(row) = &table.children[1] else {
            panic!("expected argument row")
        };
        let Node::TableCell(description) = &row.children[1] else {
            panic!("expected description cell")
        };

        assert_eq!(inline_nodes_to_markdown(&description.children), "`a\\|b`");
        assert!(
            description
                .children
                .iter()
                .any(|node| matches!(node, Node::InlineCode(code) if code.value == "a\\|b"))
        );
    }

    #[test]
    fn pipe_table_escapes_literal_pipes_in_argument_names() {
        let document = RdDocument::new(vec![tagged(
            RdTag::Arguments,
            vec![described_item(vec![text("a|b")], vec![text("description")])],
        )]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));

        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        let Node::TableRow(row) = &table.children[1] else {
            panic!("expected argument row")
        };
        let Node::TableCell(argument) = &row.children[0] else {
            panic!("expected argument cell")
        };

        assert!(
            matches!(argument.children.as_slice(), [Node::InlineCode(code)] if code.value == "a\\|b")
        );
        assert_eq!(inline_nodes_to_markdown(&argument.children), "`a\\|b`");
    }

    #[test]
    fn pipe_table_flattens_and_escapes_nested_conditional_paragraphs() {
        // A multi-node \ifelse branch collapses into a Node::Paragraph (see
        // inline::collapse_inline_nodes); table-cell content must flatten it
        // away entirely (a Paragraph is a block node and structurally unsafe
        // as a TableCell child in the real writer) and still escape embedded
        // `|` characters.
        let multi_node_conditional = tagged(
            RdTag::IfElse,
            vec![
                group(vec![text("text")]),
                group(vec![text("a|b "), tagged(RdTag::Code, vec![text("c|d")])]),
                group(vec![text("else")]),
            ],
        );
        let document = argument_document_with_description(vec![multi_node_conditional]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));

        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        let Node::TableRow(row) = &table.children[1] else {
            panic!("expected argument row")
        };
        let Node::TableCell(description) = &row.children[1] else {
            panic!("expected description cell")
        };

        // Structural invariant: no Paragraph (block node) may survive as a
        // TableCell child, at any nesting depth.
        fn assert_no_paragraphs(nodes: &[Node]) {
            for node in nodes {
                assert!(
                    !matches!(node, Node::Paragraph(_)),
                    "unexpected Paragraph in table-cell content: {node:?}"
                );
                let children: &[Node] = match node {
                    Node::Emphasis(e) => &e.children,
                    Node::Strong(s) => &s.children,
                    Node::Link(l) => &l.children,
                    _ => &[],
                };
                assert_no_paragraphs(children);
            }
        }
        assert_no_paragraphs(&description.children);

        // Render through the real writer -- not the file-local
        // inline_nodes_to_markdown helper, whose own Paragraph-unwrapping
        // fallback would mask exactly this class of bug -- and confirm the
        // row stays a single line with pipes escaped.
        let markdown = mdast_to_qmd(
            &Root::new(vec![Node::Table(table.clone())]),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );
        let row_line = markdown
            .lines()
            .find(|line| line.contains("value"))
            .expect("expected the argument row line");
        assert!(
            !row_line.contains("a|b") && row_line.contains(r"a\|b"),
            "expected escaped pipe in flattened paragraph text, got: {markdown:?}"
        );
        assert!(
            !row_line.contains("c|d") && row_line.contains(r"c\|d"),
            "expected escaped pipe in flattened paragraph inline code, got: {markdown:?}"
        );
    }

    #[test]
    fn pipe_table_sanitizer_escapes_literal_pipes_in_image_urls() {
        let sanitized = sanitize_table_cell_inline_node(&Node::image("path|name.png", "alt"));

        assert!(
            matches!(sanitized, Node::Image(image) if image.url == "path\\|name.png" && image.alt == "alt")
        );
    }

    #[test]
    fn pipe_table_sanitizer_replaces_inline_code_line_endings() {
        let sanitized = sanitize_table_cell_inline_node(&Node::inline_code("first\n second"));

        assert!(matches!(sanitized, Node::InlineCode(code) if code.value == "first  second"));
    }

    #[test]
    fn pipe_table_collapses_multiline_argument_names_through_real_writer() {
        let document = RdDocument::new(vec![tagged(
            RdTag::Arguments,
            vec![described_item(
                vec![text("alpha\n beta")],
                vec![text("description")],
            )],
        )]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );

        let row = markdown
            .lines()
            .find(|line| line.contains("description"))
            .expect("expected argument row");
        assert_eq!(row, "| `alpha beta` | description |");
    }

    #[test]
    fn pipe_table_escapes_literal_pipes_in_resolved_links() {
        let document = argument_document_with_description(vec![tagged_with_option(
            RdTag::Link,
            "=alias",
            vec![text("a|b")],
        )]);
        let arguments: Vec<_> = document.arguments().collect();
        let alias_map = HashMap::from([("alias".to_owned(), "target|variant".to_owned())]);
        let context = BlockConversionContext {
            inline: InlineConversionContext {
                links: LinkResolutionContext {
                    internal_link_url: Some("{file}.qmd#{topic}"),
                    alias_map: Some(&alias_map),
                    ..LinkResolutionContext::default()
                },
                include_html_output: false,
                prefer_ascii_math: false,
            },
            prefer_ascii_math: false,
            enclosing_heading_depth: 2,
        };
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context);

        let [Node::Table(table)] = converted.as_slice() else {
            panic!("expected one table")
        };
        let Node::TableRow(row) = &table.children[1] else {
            panic!("expected argument row")
        };
        let Node::TableCell(description) = &row.children[1] else {
            panic!("expected description cell")
        };

        assert_eq!(
            inline_nodes_to_markdown(&description.children),
            "[`a\\|b`](target\\|variant.qmd#alias)"
        );
        assert!(description.children.iter().any(|node| {
            matches!(
                node,
                Node::Link(link)
                    if link.url == "target\\|variant.qmd#alias"
                        && matches!(link.children.as_slice(), [Node::InlineCode(code)] if code.value == "a\\|b")
            )
        }));
    }

    #[test]
    fn converts_arguments_to_grid_table_with_header_separator() {
        let document = argument_document();
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::GridTable, &context(false));
        let table = html_value(&converted);

        assert!(table.contains("Argument"));
        assert!(table.contains("Description"));
        assert!(table.contains("First paragraph"));
        assert!(table.contains("Second paragraph"));
        assert!(table.contains("- one"));
        assert!(table.contains("- two"));
        assert!(table.lines().any(|line| line.contains('=')
            && line.chars().all(|character| matches!(character, '+' | '='))));
    }

    #[test]
    fn grid_table_preserves_block_equations() {
        let document = argument_document_with_description(vec![equation("x^2 + y^2", None)]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::GridTable, &context(false));
        let table = html_value(&converted);

        assert!(table.contains("$$"));
        assert!(table.contains("x^2 + y^2"));
    }

    #[test]
    fn grid_table_preserves_nested_definition_lists() {
        let describe = tagged(
            RdTag::Describe,
            vec![described_item(
                vec![text("nested term")],
                vec![text("nested description")],
            )],
        );
        let document = argument_document_with_description(vec![describe]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::GridTable, &context(false));
        let table = html_value(&converted);

        assert!(table.contains("nested term"));
        assert!(table.contains("nested description"));
    }

    #[test]
    fn converts_arguments_to_quarto_list_table() {
        let document = argument_document();
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::ListTable, &context(false));
        let table = html_value(&converted);

        assert!(table.starts_with("::: {.list-table header-rows=1}\n\n"));
        assert!(table.contains("- - Argument\n  - Description\n"));
        assert!(table.contains("- - `alpha`\n  - First paragraph\n\n    Second paragraph\n"));
        assert!(table.contains("- - `choice`\n  - Choices:\n\n    - one\n    - two\n"));
        assert!(table.ends_with("\n:::\n"));
    }

    #[test]
    fn list_table_preserves_indented_block_equations() {
        let document = argument_document_with_description(vec![equation("x^2 + y^2", None)]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::ListTable, &context(false));
        let table = html_value(&converted);

        assert!(table.contains("  - \n\n    $$\n    x^2 + y^2\n    $$\n"));
    }

    #[test]
    fn converts_arguments_to_loose_list_with_two_space_continuations() {
        let document = argument_document();
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::List, &context(false));
        let list = html_value(&converted);

        assert!(list.contains("- **`alpha`**\n\n  First paragraph\n\n  Second paragraph\n"));
        assert!(list.contains("- **`choice`**\n\n  Choices:\n\n  - one\n  - two\n"));
    }

    #[test]
    fn loose_list_preserves_indented_block_equations() {
        let document = argument_document_with_description(vec![equation("x^2 + y^2", None)]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::List, &context(false));
        let list = html_value(&converted);

        assert!(list.contains("- **`value`**\n\n  \n\n  $$\n  x^2 + y^2\n  $$\n"));
    }

    #[test]
    fn empty_arguments_return_no_nodes() {
        assert!(convert_arguments(&[], ArgumentsFormat::PipeTable, &context(false)).is_empty());
    }

    #[test]
    fn pipe_table_preserves_describe_tabular_and_preformatted_content() {
        // Regression test for Bug E: these three block shapes used to be
        // silently discarded by `flatten_for_table_cell`'s catch-all arm.
        let describe = tagged(
            RdTag::Describe,
            vec![described_item(
                vec![text("nested term")],
                vec![text("nested description")],
            )],
        );
        let table = tagged(
            RdTag::Tabular,
            vec![group(vec![text("l")]), group(vec![text("table cell text")])],
        );
        let preformatted = tagged(RdTag::Preformatted, vec![text("preformatted text")]);
        let document = argument_document_with_description(vec![describe, table, preformatted]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );
        let row = markdown
            .lines()
            .find(|line| line.contains("value"))
            .expect("expected the argument row line");

        assert!(
            row.contains("nested term"),
            "describe term missing, got: {row:?}"
        );
        assert!(
            row.contains("nested description"),
            "describe description missing, got: {row:?}"
        );
        assert!(
            row.contains("table cell text"),
            "tabular content missing, got: {row:?}"
        );
        assert!(
            row.contains("preformatted text"),
            "preformatted content missing, got: {row:?}"
        );
    }

    #[test]
    fn pipe_table_preserves_both_paragraphs_of_a_multi_paragraph_list_item() {
        // Regression test for Bug B (pipe-table location): only the first
        // paragraph of a multi-paragraph `\itemize` item used to survive.
        let list = delimited_list(
            RdTag::Itemize,
            vec![vec![text("first paragraph\n\nsecond paragraph")]],
        );
        let document = argument_document_with_description(vec![list]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );
        let row = markdown
            .lines()
            .find(|line| line.contains("value"))
            .expect("expected the argument row line");

        assert!(row.contains("first paragraph"), "got: {row:?}");
        assert!(row.contains("second paragraph"), "got: {row:?}");
    }

    #[test]
    fn grid_table_preserves_tabular_content() {
        // Regression test for Bug A: `Node::Table` used to be silently
        // discarded by `nodes_to_markdown`'s catch-all arm.
        let table = tagged(
            RdTag::Tabular,
            vec![group(vec![text("l")]), group(vec![text("table cell text")])],
        );
        let document = argument_document_with_description(vec![table]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::GridTable, &context(false));
        let table_text = html_value(&converted);

        assert!(
            table_text.contains("table cell text"),
            "got: {table_text:?}"
        );
    }

    #[test]
    fn grid_table_preserves_both_paragraphs_of_a_multi_paragraph_list_item() {
        // Regression test for Bug B (grid-table location): only the first
        // paragraph of a multi-paragraph `\itemize` item used to survive.
        let list = delimited_list(
            RdTag::Itemize,
            vec![vec![text("first paragraph\n\nsecond paragraph")]],
        );
        let document = argument_document_with_description(vec![list]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::GridTable, &context(false));
        let table_text = html_value(&converted);

        assert!(
            table_text.contains("first paragraph"),
            "got: {table_text:?}"
        );
        assert!(
            table_text.contains("second paragraph"),
            "got: {table_text:?}"
        );
    }

    #[test]
    fn grid_table_list_item_second_paragraph_is_indented_under_marker() {
        // Regression test for Bug 1: a list item's continuation content
        // (here, a second paragraph) must be indented under the marker
        // column, or a grid-table cell's block-level Markdown parser stops
        // treating it as part of the same list item.
        let list = delimited_list(
            RdTag::Itemize,
            vec![vec![text("first paragraph\n\nsecond paragraph")]],
        );
        let markdown = convert_to_markdown_text(&[list], &context(false));

        assert_eq!(markdown, "- first paragraph\n\n  second paragraph");
    }

    #[test]
    fn grid_table_list_item_cr_break_in_first_paragraph_is_indented_under_marker() {
        // Regression test for the latent bug Bug 1's fix also closes: a
        // `\cr`-induced line break *within the first paragraph* of a list
        // item must also be indented under the marker, not just subsequent
        // sibling blocks.
        let list = delimited_list(
            RdTag::Itemize,
            vec![vec![
                text("first line"),
                tagged(RdTag::Cr, vec![]),
                text("second line"),
            ]],
        );
        let markdown = convert_to_markdown_text(&[list], &context(false));

        assert_eq!(markdown, "- first line  \n  second line");
    }

    #[test]
    fn pipe_table_nested_tabular_pipe_is_not_double_escaped() {
        // Regression test for Bug 2: `flatten_block_node_for_table_cell`'s
        // blind `.replace('|', "\\|")` used to re-escape a `|` that
        // `convert_tabular`'s cell sanitization had already escaped once,
        // corrupting the outer pipe-table row once the doubled backslash
        // was itself consumed by CommonMark's backslash-escape processing.
        let table = tagged(
            RdTag::Tabular,
            vec![group(vec![text("l")]), group(vec![text("a | b")])],
        );
        let document = RdDocument::new(vec![tagged(
            RdTag::Arguments,
            vec![described_item(vec![text("table_arg")], vec![table])],
        )]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );
        let row = markdown
            .lines()
            .find(|line| line.contains("table_arg"))
            .expect("expected the argument row line");

        assert!(
            row.starts_with("| `table_arg` |"),
            "argument-name column boundary was corrupted, got: {row:?}"
        );
        assert_eq!(
            count_unescaped_pipes(row),
            3,
            "expected exactly 3 unescaped pipes (outer 2-column row delimiters), got: {row:?}"
        );
    }

    /// Count `|` characters in `line` that are not already escaped by an
    /// odd-length run of preceding backslashes -- i.e. the delimiters a
    /// GFM pipe-table parser would actually split columns on.
    fn count_unescaped_pipes(line: &str) -> usize {
        let mut count = 0;
        let mut backslash_run = 0usize;
        for character in line.chars() {
            if character == '|' {
                if backslash_run.is_multiple_of(2) {
                    count += 1;
                }
                backslash_run = 0;
            } else if character == '\\' {
                backslash_run += 1;
            } else {
                backslash_run = 0;
            }
        }
        count
    }

    #[test]
    fn pipe_table_tabular_nested_in_describe_pipe_is_not_double_escaped() {
        // Regression test for roborev job 264: `flatten_block_node_for_table_cell`'s
        // `DefinitionList` arm used to serialize the whole `\describe{}`
        // subtree to one string and blind-escape it, doubling the escape on
        // a `Table` nested inside a `\describe` item's body (already
        // escaped once by `convert_tabular`'s cell sanitization) and
        // corrupting the outer pipe-table row once the doubled backslash
        // was itself consumed by CommonMark's backslash-escape processing.
        let table = tagged(
            RdTag::Tabular,
            vec![group(vec![text("l")]), group(vec![text("a | b")])],
        );
        let describe = tagged(
            RdTag::Describe,
            vec![described_item(vec![text("key")], vec![table])],
        );
        let document = RdDocument::new(vec![tagged(
            RdTag::Arguments,
            vec![described_item(vec![text("describe_arg")], vec![describe])],
        )]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );
        let row = markdown
            .lines()
            .find(|line| line.contains("describe_arg"))
            .expect("expected the argument row line");

        assert!(
            row.starts_with("| `describe_arg` |"),
            "argument-name column boundary was corrupted, got: {row:?}"
        );
        assert_eq!(
            count_unescaped_pipes(row),
            3,
            "expected exactly 3 unescaped pipes (outer 2-column row delimiters), got: {row:?}"
        );
    }

    #[test]
    fn pipe_table_preformatted_pipe_preserves_backslash() {
        // Regression test for roborev job 263: unlike a nested `\tabular`
        // (already escaped once by `sanitize_table_cell_inline_nodes`),
        // `\preformatted{}` content is raw and never pre-escaped, so a
        // literal `\|` in the source is genuine data (e.g. a regex or file
        // path) that must survive intact -- not table-generated escaping to
        // be left alone. Verified empirically against Pandoc 3.7 that a
        // *blind* re-escape (`\|` -> `\\|`) is what's required here: the
        // pipe-table row splitter consumes exactly one backslash per `\|`
        // before the code span is parsed, so anything less loses the
        // original backslash from the rendered code.
        let preformatted = tagged(RdTag::Preformatted, vec![text(r"a\|b")]);
        let document = argument_document_with_description(vec![preformatted]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );

        assert!(
            markdown.contains(r"`a\\|b`"),
            "expected the literal backslash to survive in the rendered code span, got: {markdown:?}"
        );
    }

    #[test]
    fn pipe_table_math_pipe_preserves_backslash() {
        // Regression test for roborev job 263: a `\deqn` block equation's
        // LaTeX source is also never pre-escaped, so a literal `\|` (e.g.
        // semantic TeX) must be preserved the same way as preformatted
        // content, not treated as already-escaped table syntax.
        let deqn = equation(r"a\|b", None);
        let document = argument_document_with_description(vec![deqn]);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::PipeTable, &context(false));
        let markdown = mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        );

        assert!(
            markdown.contains(r"a\\|b"),
            "expected the literal backslash to survive in the rendered math, got: {markdown:?}"
        );
    }

    #[test]
    fn grid_table_escapes_list_marker_lookalikes_after_cr_break() {
        // Regression test for Bug C: `\cr`-separated continuation lines that
        // start with a Markdown list-marker-lookalike must be escaped so
        // Pandoc doesn't reparse them as a nested list inside the grid-table
        // cell.
        let description = vec![
            text("First line."),
            tagged(RdTag::Cr, vec![]),
            text(" - hyphen."),
            tagged(RdTag::Cr, vec![]),
            text(" * asterisk."),
            tagged(RdTag::Cr, vec![]),
            text(" + plus."),
            tagged(RdTag::Cr, vec![]),
            text(" 1. ordered period."),
        ];
        let document = argument_document_with_description(description);
        let arguments: Vec<_> = document.arguments().collect();
        let converted = convert_arguments(&arguments, ArgumentsFormat::GridTable, &context(false));
        let table_text = html_value(&converted);

        assert!(table_text.contains(r"\- hyphen."), "got: {table_text:?}");
        assert!(table_text.contains(r"\* asterisk."), "got: {table_text:?}");
        assert!(table_text.contains(r"\+ plus."), "got: {table_text:?}");
        assert!(
            table_text.contains(r"1\. ordered period."),
            "got: {table_text:?}"
        );
    }
}
