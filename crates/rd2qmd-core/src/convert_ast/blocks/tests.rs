use std::collections::HashMap;

use rd_ast::{RdDocument, RdNode, RdTag};
use rd2qmd_mdast::{Node, Root, WriterOptions, mdast_to_qmd};

use super::markdown_text::{convert_to_markdown_text, inline_nodes_to_markdown};
use super::table_cell::sanitize_table_cell_inline_node;
use super::{
    BlockConversionContext, convert_arguments, convert_block_content, convert_custom_section,
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
    let document = argument_document_with_description(vec![tagged(RdTag::Code, vec![text("a|b")])]);
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
    assert!(
        table.lines().any(|line| line.contains('=')
            && line.chars().all(|character| matches!(character, '+' | '=')))
    );
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
