use super::*;
use crate::mdast::*;
use crate::writer::{ArgumentsFormat, RdMetadata};

fn typst(nodes: Vec<Node>) -> String {
    mdast_to_typst(&Root::new(nodes), &TypstWriterOptions::default())
}

fn typst_with(nodes: Vec<Node>, options: &TypstWriterOptions) -> String {
    mdast_to_typst(&Root::new(nodes), options)
}

#[test]
fn writes_headings_and_paragraphs() {
    let output = typst(vec![
        Node::heading(1, vec![Node::text("Title")]),
        Node::paragraph(vec![Node::text("Body text.")]),
        Node::heading(3, vec![Node::text("Deep")]),
    ]);
    assert_eq!(output, "= Title\n\nBody text\\.\n\n=== Deep\n");
}

#[test]
fn writes_emphasis_strong_and_inline_code() {
    let output = typst(vec![Node::paragraph(vec![
        Node::emphasis(vec![Node::text("em")]),
        Node::text(" "),
        Node::strong(vec![Node::text("strong")]),
        Node::text(" "),
        Node::inline_code("x <- 1"),
    ])]);
    assert_eq!(output, "#emph[em] #strong[strong] `x <- 1`\n");
}

#[test]
fn emphasis_and_strong_are_safe_next_to_word_characters() {
    let output = typst(vec![Node::paragraph(vec![
        Node::text("non"),
        Node::emphasis(vec![Node::text("linear")]),
        Node::text(" and word"),
        Node::strong(vec![Node::text("strong")]),
    ])]);
    assert_eq!(output, "non#emph[linear] and word#strong[strong]\n");
}

#[test]
fn emphasis_is_safe_before_parenthesized_text() {
    let output = typst(vec![Node::paragraph(vec![
        Node::emphasis(vec![Node::text("111")]),
        Node::text("(9)"),
    ])]);
    assert_eq!(output, "#emph[111]\\(9\\)\n");
}

#[test]
fn escapes_typst_syntax_characters_in_text() {
    let output = typst(vec![Node::paragraph(vec![Node::text(
        "a #b $c *d _e `f <g> @h ~i [j]",
    )])]);
    assert_eq!(
        output,
        "a \\#b \\$c \\*d \\_e \\`f \\<g\\> \\@h \\~i \\[j\\]\n"
    );
}

#[test]
fn escapes_only_line_leading_markup_characters() {
    // `-`, `+`, `=`, `/` and `1.` start a list, heading or term at the start
    // of a line, but are ordinary text anywhere else.
    let output = typst(vec![
        Node::paragraph(vec![Node::text("- leading hyphen")]),
        Node::paragraph(vec![Node::text("mid - hyphen")]),
        Node::paragraph(vec![Node::text("1. ordered")]),
        Node::paragraph(vec![Node::text("value 1. mid")]),
    ]);
    assert_eq!(
        output,
        "\\- leading hyphen\n\nmid - hyphen\n\n1\\. ordered\n\nvalue 1\\. mid\n"
    );
}

#[test]
fn a_line_break_restores_line_start_escaping() {
    let output = typst(vec![Node::paragraph(vec![
        Node::text("first"),
        Node::line_break(),
        Node::text("- second"),
    ])]);
    assert_eq!(output, "first \\\n\\- second\n");
}

#[test]
fn writes_links_and_images() {
    let output = typst(vec![Node::paragraph(vec![
        Node::link("https://example.com/a.html", vec![Node::text("label")]),
        Node::text(" "),
        Node::image("figure.png", "alt text"),
    ])]);
    assert_eq!(
        output,
        "#link(\"https://example.com/a.html\")[label] #image(\"figure.png\", alt: \"alt text\")\n"
    );
}

#[test]
fn escapes_periods_after_typst_function_calls_and_dash_shorthands() {
    let output = typst(vec![Node::paragraph(vec![
        Node::link("https://example.com", vec![Node::text("site")]),
        Node::text(".tar.gz --vanilla 1---3"),
    ])]);
    assert_eq!(
        output,
        "#link(\"https://example.com\")[site]\\.tar\\.gz \\-\\-vanilla 1\\-\\-\\-3\n"
    );
}

#[test]
fn a_self_labelled_link_needs_no_content_block() {
    let output = typst(vec![Node::paragraph(vec![Node::link(
        "https://example.com",
        vec![Node::text("https://example.com")],
    )])]);
    assert_eq!(output, "#link(\"https://example.com\")\n");
}

#[test]
fn writes_r_code_as_a_plain_raw_block() {
    // Never `{r}`: Typst has no executable blocks, and calepin executes a
    // plain ```r block.
    let output = typst(vec![Node::code_with_meta(
        Some("r".to_owned()),
        Some("executable".to_owned()),
        "plot(1:10)",
    )]);
    assert_eq!(output, "```r\nplot(1:10)\n```\n");
}

#[test]
fn marks_non_executable_r_code_for_calepin() {
    let output = typst(vec![Node::code(Some("r".to_owned()), "function(x)")]);
    assert_eq!(output, "```r\n#| eval: false\nfunction(x)\n```\n");
}

#[test]
fn hides_setup_code_but_keeps_it_executable_for_calepin() {
    let output = typst(vec![Node::code_with_meta(
        Some("r".to_owned()),
        Some("hidden".to_owned()),
        "setup()",
    )]);
    assert_eq!(
        output,
        "```r\n#| echo: false\n#| results: hide\nsetup()\n```\n"
    );
}

#[test]
fn code_containing_a_fence_falls_back_to_the_raw_function() {
    // Typst raw blocks have no longer-fence escape hatch.
    let output = typst(vec![Node::code_with_meta(
        Some("r".to_owned()),
        Some("executable".to_owned()),
        "x <- \"```triple```\"",
    )]);
    assert_eq!(
        output,
        "#raw(block: true, lang: \"r\", \"x <- \\\"```triple```\\\"\")\n"
    );
}

#[test]
fn inline_code_containing_a_backtick_falls_back_to_the_raw_function() {
    let output = typst(vec![Node::paragraph(vec![Node::inline_code("`a`")])]);
    assert_eq!(output, "#raw(\"`a`\")\n");
}

#[test]
fn writes_latex_math_through_mitex_and_imports_it_once() {
    let output = typst(vec![
        Node::paragraph(vec![Node::inline_math("\\alpha_i")]),
        Node::math("\\sum_{i=1}^{n} x_i"),
    ]);
    assert_eq!(
        output,
        format!(
            "#import \"@preview/mitex:{MITEX_VERSION}\": mi, mitex\n\n\
             #mi(`\\alpha_i`)\n\n\
             #mitex(`\\sum_{{i=1}}^{{n}} x_i`)\n"
        )
    );
}

#[test]
fn a_document_without_math_does_not_import_mitex() {
    let output = typst(vec![Node::paragraph(vec![Node::text("no math")])]);
    assert!(!output.contains("mitex"));
}

#[test]
fn writes_lists_with_nesting() {
    let output = typst(vec![Node::list(
        false,
        vec![
            Node::list_item(vec![
                Node::paragraph(vec![Node::text("first")]),
                Node::list(
                    true,
                    vec![Node::list_item(vec![Node::paragraph(vec![Node::text(
                        "nested",
                    )])])],
                ),
            ]),
            Node::list_item(vec![Node::paragraph(vec![Node::text("second")])]),
        ],
    )]);
    assert_eq!(output, "- first\n  1. nested\n- second\n");
}

#[test]
fn preserves_paragraph_boundaries_inside_list_items() {
    let output = typst(vec![Node::list(
        false,
        vec![Node::list_item(vec![
            Node::paragraph(vec![Node::text("first")]),
            Node::paragraph(vec![Node::text("second")]),
        ])],
    )]);
    assert_eq!(output, "- first\n\n  second\n");
}

#[test]
fn writes_a_tabular_as_a_typst_table() {
    let output = typst(vec![Node::table(
        vec![Some(Align::Left), Some(Align::Right)],
        vec![
            Node::table_row(vec![
                Node::table_cell(vec![Node::text("a")]),
                Node::table_cell(vec![Node::text("1")]),
            ]),
            Node::table_row(vec![
                Node::table_cell(vec![Node::text("b")]),
                Node::table_cell(vec![Node::text("2")]),
            ]),
        ],
    )]);
    assert_eq!(
        output,
        "#table(\n  columns: 2,\n  align: (left, right,),\n  [a], [1],\n  [b], [2],\n)\n"
    );
}

#[test]
fn writes_a_definition_list_as_typst_terms() {
    let output = typst(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::inline_code("a")]),
        Node::definition_description(vec![Node::paragraph(vec![Node::text("first")])]),
    ])]);
    assert_eq!(output, "#terms(\n  terms.item([`a`], [first]),\n)\n");
}

fn arguments_node() -> Node {
    Node::arguments(vec![
        ArgumentItem {
            name: "x".to_owned(),
            description: vec![Node::paragraph(vec![Node::text("A number.")])],
        },
        ArgumentItem {
            name: "y".to_owned(),
            description: vec![
                Node::paragraph(vec![Node::text("Choices:")]),
                Node::list(
                    false,
                    vec![Node::list_item(vec![Node::paragraph(vec![Node::text(
                        "one",
                    )])])],
                ),
            ],
        },
    ])
}

#[test]
fn writes_arguments_as_a_table_holding_block_content() {
    let output = typst(vec![arguments_node()]);
    assert_eq!(
        output,
        "#table(\n  \
           columns: 2,\n  \
           table.header([Argument], [Description]),\n  \
           [`x`], [A number\\.],\n  \
           [`y`], [\n    Choices:\n\n    - one\n  ],\n\
         )\n"
    );
}

#[test]
fn every_markdown_table_format_maps_to_one_typst_table() {
    // The pipe/grid/list-table split exists only to work around Markdown
    // table limitations that Typst's table does not have.
    let table = typst_with(
        vec![arguments_node()],
        &TypstWriterOptions {
            arguments_format: ArgumentsFormat::PipeTable,
            ..TypstWriterOptions::default()
        },
    );
    for format in [ArgumentsFormat::GridTable, ArgumentsFormat::ListTable] {
        assert_eq!(
            typst_with(
                vec![arguments_node()],
                &TypstWriterOptions {
                    arguments_format: format,
                    ..TypstWriterOptions::default()
                },
            ),
            table
        );
    }
}

#[test]
fn the_list_arguments_format_maps_to_typst_terms() {
    let output = typst_with(
        vec![arguments_node()],
        &TypstWriterOptions {
            arguments_format: ArgumentsFormat::List,
            ..TypstWriterOptions::default()
        },
    );
    assert!(output.starts_with("#terms(\n  terms.item[`x`][A number\\.],\n"));
}

#[test]
fn writes_a_simple_out_fragment_as_a_guarded_html_elem() {
    let output = typst(vec![Node::paragraph(vec![
        Node::html("<sup>2</sup>"),
        Node::text(" "),
        Node::html("<span class=\"x\">tagged</span>"),
        Node::text(" "),
        Node::html("<br>"),
    ])]);
    assert_eq!(
        output,
        "#context { if target() == \"html\" { html.elem(\"sup\")[2] } } \
         #context { if target() == \"html\" { html.elem(\"span\", attrs: (class: \"x\"))[tagged] } } \
         #context { if target() == \"html\" { html.elem(\"br\") } }\n"
    );
}

#[test]
fn decodes_html_entities_in_text_elements_and_attributes() {
    let output = typst(vec![Node::paragraph(vec![
        Node::html("Tom &amp; Jerry"),
        Node::text(" "),
        Node::html("<span title=\"A &amp; B\">&#169; 2026</span>"),
    ])]);
    assert_eq!(
        output,
        "Tom & Jerry #context { if target() == \"html\" { html.elem(\"span\", attrs: (title: \"A & B\"))[© 2026] } }\n"
    );
}

#[test]
fn keeps_a_complex_html_fragment_verbatim_instead_of_dropping_it() {
    let output = typst(vec![Node::paragraph(vec![Node::html(
        "<div><p>nested</p></div>",
    )])]);
    assert_eq!(output, "#raw(\"<div><p>nested</p></div>\")\n");
}

#[test]
fn writes_frontmatter_as_document_metadata_and_a_title_heading() {
    let output = typst_with(
        vec![Node::paragraph(vec![Node::text("body")])],
        &TypstWriterOptions {
            frontmatter: Some(Frontmatter {
                title: Some("The Title".to_owned()),
                pagetitle: Some("The Title — topic".to_owned()),
                format: None,
                metadata: Some(RdMetadata {
                    lifecycle: Some("experimental".to_owned()),
                    aliases: vec!["topic".to_owned(), "topic2".to_owned()],
                    source_files: vec!["R/topic.R".to_owned()],
                    ..RdMetadata::default()
                }),
            }),
            ..TypstWriterOptions::default()
        },
    );
    assert_eq!(
        output,
        "#set document(title: \"The Title\")\n\
         #metadata((\n  \
           pagetitle: \"The Title — topic\",\n  \
           lifecycle: \"experimental\",\n  \
           aliases: (\"topic\", \"topic2\"),\n  \
           \"source-files\": (\"R/topic.R\",),\n\
         ))<rd2qmd>\n\n\
         = The Title\n\n\
         body\n"
    );
}
