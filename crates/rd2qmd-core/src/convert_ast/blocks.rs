//! General block-content assembly for the rd_ast conversion migration.

use rd_ast::{RdArgument, RdListItem, RdListKind, RdNode, RdPath, RdTag};
use rd2qmd_mdast::{Align, Html, Node, Root, WriterOptions, mdast_to_qmd};
use tabled::settings::Style;
use tabled::settings::style::HorizontalLine;

use super::{
    inline::{self, LinkResolutionContext},
    leaf_text::flatten_verbatim_leaves,
    traversal::{BlockContentItem, ParagraphItem, scan_block_content},
};

/// Borrowed configuration used while converting general block content.
#[derive(Clone, Copy)]
pub(crate) struct BlockConversionContext<'a> {
    pub(crate) links: LinkResolutionContext<'a>,
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
            #[cfg(feature = "roxygen")]
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
        inline::convert_inline_nodes(section.title, &context.links),
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
        let term_text = argument_name(argument, context).replace('|', "\\|");
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
    inline::extract_plain_text(&inline::convert_inline_nodes(argument.name, &context.links))
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
                        for child in &item.children {
                            if let Node::Paragraph(paragraph) = child {
                                extend_table_cell_inline(&mut result, &paragraph.children);
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    result
}

fn extend_table_cell_inline(result: &mut Vec<Node>, nodes: &[Node]) {
    result.extend(nodes.iter().map(sanitize_table_cell_inline_node));
}

fn sanitize_table_cell_inline_node(node: &Node) -> Node {
    let mut node = node.clone();
    match &mut node {
        Node::Text(text) => text.value = text.value.replace('|', "\\|"),
        Node::InlineCode(code) => code.value = code.value.replace('|', "\\|"),
        Node::InlineMath(math) => math.value = math.value.replace('|', "\\|"),
        // Accepted limitation: this collapse is not TeX-comment-aware, so a `%` line
        // comment can swallow later `\deqn` content; handling `\%` needs TeX-aware
        // stripping for this narrow `\deqn`-in-`\tabular` case.
        Node::Math(math) => {
            return Node::InlineMath(rd2qmd_mdast::InlineMath {
                value: math
                    .value
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .replace('|', "\\|"),
            });
        }
        Node::Image(image) => {
            image.url = image.url.replace('|', "\\|");
            image.alt = image.alt.replace('|', "\\|");
            if let Some(title) = &mut image.title {
                *title = title.replace('|', "\\|");
            }
        }
        Node::Html(html) => html.value = html.value.replace('|', "\\|"),
        Node::Break => {
            return Node::Html(Html {
                value: "<br>".to_owned(),
            });
        }
        Node::Emphasis(emphasis) => {
            emphasis.children = emphasis
                .children
                .iter()
                .map(sanitize_table_cell_inline_node)
                .collect();
        }
        Node::Strong(strong) => {
            strong.children = strong
                .children
                .iter()
                .map(sanitize_table_cell_inline_node)
                .collect();
        }
        Node::Link(link) => {
            link.url = link.url.replace('|', "\\|");
            link.children = link
                .children
                .iter()
                .map(sanitize_table_cell_inline_node)
                .collect();
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
                result.push_str(&inline_nodes_to_markdown(&paragraph.children));
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
                        result.push_str(&marker);
                        for child in &item.children {
                            if let Node::Paragraph(paragraph) = child {
                                result.push_str(&inline_nodes_to_markdown(&paragraph.children));
                                break;
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
            Node::Math(_) | Node::DefinitionList(_) => {
                result.push_str(&node_to_markdown_string(node));
            }
            _ => {
                if let Some(text) = node_to_text(node) {
                    result.push_str(&text);
                }
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
                result.push_str(&link.url);
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
                result.push_str(&image.url);
                if let Some(title) = &image.title {
                    result.push_str(" \"");
                    result.push_str(title);
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
                inline::convert_inline_nodes(std::slice::from_ref(node), &context.links)
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
                    &context.links,
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
            vec![Node::math(equation_text(equation.latex(), &context.links))]
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
            let cells = row
                .cells()
                .iter()
                .map(|cell| {
                    let children = inline::convert_inline_nodes(cell.nodes(), &context.links)
                        .iter()
                        .map(sanitize_table_cell_inline_node)
                        .collect();
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
        inline::convert_inline_nodes(title.children(), &context.links),
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

fn equation_text(nodes: &[RdNode], links: &LinkResolutionContext<'_>) -> String {
    inline::extract_plain_text(&inline::convert_inline_nodes(nodes, links))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rd_ast::{RdDocument, RdNode, RdTag};
    use rd2qmd_mdast::Node;

    use super::{
        BlockConversionContext, convert_arguments, convert_block_content, convert_custom_section,
        inline_nodes_to_markdown, sanitize_table_cell_inline_node,
    };
    use crate::ArgumentsFormat;
    use crate::convert_ast::document::build_custom_sections;
    use crate::convert_ast::inline::LinkResolutionContext;

    fn context(prefer_ascii_math: bool) -> BlockConversionContext<'static> {
        BlockConversionContext {
            links: LinkResolutionContext::default(),
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
    fn pipe_table_sanitizer_escapes_literal_pipes_in_image_urls() {
        let sanitized = sanitize_table_cell_inline_node(&Node::image("path|name.png", "alt"));

        assert!(
            matches!(sanitized, Node::Image(image) if image.url == "path\\|name.png" && image.alt == "alt")
        );
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
            links: LinkResolutionContext {
                internal_link_url: Some("{file}.qmd#{topic}"),
                alias_map: Some(&alias_map),
                ..LinkResolutionContext::default()
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
}
