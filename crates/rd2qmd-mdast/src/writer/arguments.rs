//! Markdown rendering of an [`Arguments`] section.
//!
//! The Arguments section is the one construct whose physical shape is a user
//! choice ([`ArgumentsFormat`]), so the converter hands the writer a semantic
//! node and every rendering decision is made here.
//!
//! The standalone markdown-string serializers below are used for grid-table /
//! list-table cells -- a separate code path from the pipe-table flattening in
//! [`super::table_cell`], per the doc comment on [`convert_to_markdown_text`].

use tabled::settings::Style;
use tabled::settings::style::HorizontalLine;

use super::table_cell::flatten_for_table_cell;
use super::{ArgumentsFormat, Writer, WriterOptions, mdast_to_qmd};
use crate::mdast::{Align, ArgumentItem, Arguments, Node, Root};

impl Writer<'_> {
    pub(super) fn write_arguments(&mut self, arguments: &Arguments) {
        if arguments.items.is_empty() {
            return;
        }
        match self.options.arguments_format {
            ArgumentsFormat::PipeTable => self.write_arguments_pipe(&arguments.items),
            ArgumentsFormat::GridTable => self.write_arguments_grid(&arguments.items),
            ArgumentsFormat::ListTable => self.write_arguments_list_table(&arguments.items),
            ArgumentsFormat::List => self.write_arguments_list(&arguments.items),
        }
    }

    /// Pipe table: cannot contain block elements (lists, multiple paragraphs).
    /// Workaround: use `<br>` for line breaks and flatten lists with bullet markers.
    fn write_arguments_pipe(&mut self, items: &[ArgumentItem]) {
        let header_row = Node::table_row(vec![
            Node::table_cell(vec![Node::text("Argument")]),
            Node::table_cell(vec![Node::text("Description")]),
        ]);
        let mut rows = vec![header_row];

        for item in items {
            let name = replace_line_endings_with_space(&item.name).replace('|', "\\|");
            rows.push(Node::table_row(vec![
                Node::table_cell(vec![Node::inline_code(name.trim())]),
                Node::table_cell(flatten_for_table_cell(&item.description)),
            ]));
        }

        let table = crate::mdast::Table {
            align: vec![Some(Align::Left), Some(Align::Left)],
            children: rows,
        };
        // Cells are already pipe-table-sanitized by `flatten_for_table_cell`.
        self.write_table_with(&table, true);
    }

    /// Pandoc grid table: supports block elements (lists, paragraphs) in cells.
    fn write_arguments_grid(&mut self, items: &[ArgumentItem]) {
        use tabled::builder::Builder;

        let mut builder = Builder::default();
        builder.push_record(["Argument", "Description"]);

        for item in items {
            let arg_text = crate::format_inline_code(item.name.trim(), false);
            let desc_text = convert_to_markdown_text(&item.description);
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
        self.write_raw_block(&table.with(grid_style).to_string());
    }

    /// Quarto list-table: requires Quarto 1.9+, compatible with q2.
    fn write_arguments_list_table(&mut self, items: &[ArgumentItem]) {
        let mut output = String::new();
        output.push_str("::: {.list-table header-rows=1}\n\n");
        output.push_str("- - Argument\n  - Description\n");

        for item in items {
            output.push('\n');
            output.push_str("- - ");
            output.push_str(&crate::format_inline_code(item.name.trim(), false));
            output.push('\n');
            output.push_str("  - ");
            output.push_str(&render_list_table_cell(&item.description));
            output.push('\n');
        }

        output.push_str("\n:::\n");
        self.write_raw_block(&output);
    }

    /// Markdown loose list: compatible everywhere.
    fn write_arguments_list(&mut self, items: &[ArgumentItem]) {
        let mut output = String::new();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            output.push_str("- **");
            output.push_str(&crate::format_inline_code(item.name.trim(), false));
            output.push_str("**\n");

            let desc = render_block_content(&item.description, 2);
            if !desc.is_empty() {
                output.push('\n');
                output.push_str("  ");
                output.push_str(&desc);
                output.push('\n');
            }
        }
        self.write_raw_block(&output);
    }
}

/// Replace line endings with a single space, leaving every other byte of
/// whitespace untouched.
fn replace_line_endings_with_space(value: &str) -> String {
    value.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

/// Convert block nodes to a standalone Markdown string for a grid-table cell.
///
/// Grid tables are built from raw strings by `tabled`, whereas the main mdast
/// writer owns one global output buffer and tracks whole-document line state.
/// The dedicated serializers below therefore keep a separate subtree path
/// until the writer can directly serialize isolated fragments.
pub(crate) fn convert_to_markdown_text(nodes: &[Node]) -> String {
    nodes_to_markdown(nodes)
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
            Node::Arguments(_)
            | Node::ThematicBreak
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

pub(crate) fn inline_nodes_to_markdown(nodes: &[Node]) -> String {
    let mut result = String::new();

    for node in nodes {
        match node {
            Node::Text(text) => result.push_str(&text.value),
            Node::InlineCode(code) => result.push_str(&crate::format_inline_code(
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
                result.push_str(&crate::format_link_destination(&link.url));
                if let Some(title) = &link.title {
                    result.push_str(" \"");
                    result.push_str(&crate::escape_link_title(title));
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
                result.push_str(&crate::format_link_destination(&image.url));
                if let Some(title) = &image.title {
                    result.push_str(" \"");
                    result.push_str(&crate::escape_link_title(title));
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

pub(crate) fn render_list_table_cell(nodes: &[Node]) -> String {
    render_block_content(nodes, 4)
}

pub(crate) fn render_block_content(nodes: &[Node], indent: u8) -> String {
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
pub(crate) fn node_to_markdown_string(node: &Node) -> String {
    let root = Root::new(vec![node.clone()]);
    let options = WriterOptions {
        frontmatter: None,
        // The migration context has no writer-format option yet. Preserve the
        // legacy call site's default until the document converter is wired.
        quarto_code_blocks: true,
        // Unreachable from here: an Arguments node never nests inside a cell.
        arguments_format: ArgumentsFormat::default(),
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
