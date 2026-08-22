use super::*;
use crate::mdast::*;
use crate::writer::block::calculate_fence_length;

#[test]
fn test_heading() {
    let root = Root::new(vec![Node::heading(1, vec![Node::text("Title")])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert_eq!(qmd.trim(), "# Title");
}

#[test]
fn test_heading_levels() {
    for depth in 1..=6 {
        let root = Root::new(vec![Node::heading(depth, vec![Node::text("Title")])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        let expected_prefix = "#".repeat(depth as usize);
        assert!(qmd.starts_with(&format!("{} Title", expected_prefix)));
    }
}

#[test]
fn test_paragraph() {
    let root = Root::new(vec![Node::paragraph(vec![Node::text("Hello world")])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert_eq!(qmd.trim(), "Hello world");
}

#[test]
fn test_multiple_paragraphs() {
    let root = Root::new(vec![
        Node::paragraph(vec![Node::text("First paragraph")]),
        Node::paragraph(vec![Node::text("Second paragraph")]),
    ]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("First paragraph"));
    assert!(qmd.contains("Second paragraph"));
    // Paragraphs should be separated by blank line
    assert!(qmd.contains("\n\n"));
}

#[test]
fn test_code_block() {
    let root = Root::new(vec![Node::code(Some("r".to_string()), "x <- 1")]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("```r"));
    assert!(qmd.contains("x <- 1"));
    assert!(qmd.contains("```\n"));
}

#[test]
fn test_code_block_no_language() {
    let root = Root::new(vec![Node::code(None, "some code")]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("```\n"));
    assert!(qmd.contains("some code"));
}

#[test]
fn test_code_block_with_triple_backticks() {
    // Code containing triple backticks needs longer fence
    let code = "Here is a code block:\n```r\nx <- 1\n```";
    let root = Root::new(vec![Node::code(Some("markdown".to_string()), code)]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());

    // Should use 4 backticks since content has 3
    assert!(qmd.contains("````markdown"));
    assert!(qmd.contains("````\n") || qmd.ends_with("````"));
}

#[test]
fn test_code_block_with_many_backticks() {
    // Code containing 5 backticks needs 6 for fence
    let code = "`````";
    let root = Root::new(vec![Node::code(None, code)]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());

    // Should use 6 backticks
    assert!(qmd.contains("``````\n"));
}

#[test]
fn test_calculate_fence_length() {
    // No backticks -> 3
    assert_eq!(calculate_fence_length("hello"), 3);

    // 1 backtick -> 3
    assert_eq!(calculate_fence_length("hello ` world"), 3);

    // 2 backticks -> 3
    assert_eq!(calculate_fence_length("hello `` world"), 3);

    // 3 backticks -> 4
    assert_eq!(calculate_fence_length("```"), 4);

    // 3 backticks in middle -> 4
    assert_eq!(calculate_fence_length("hello\n```r\nx <- 1\n```"), 4);

    // 5 backticks -> 6
    assert_eq!(calculate_fence_length("`````"), 6);
}

#[test]
fn test_quarto_code_block() {
    // Executable code block (Examples section) uses {r}
    let root = Root::new(vec![Node::code_with_meta(
        Some("r".to_string()),
        Some("executable".to_string()),
        "x <- 1",
    )]);
    let opts = WriterOptions {
        quarto_code_blocks: true,
        ..Default::default()
    };
    let qmd = mdast_to_qmd(&root, &opts);
    assert!(qmd.contains("```{r}"));

    // Non-executable code block (Usage section) uses plain r
    let root2 = Root::new(vec![Node::code(Some("r".to_string()), "foo(x)")]);
    let qmd2 = mdast_to_qmd(&root2, &opts);
    assert!(qmd2.contains("```r"));
    assert!(!qmd2.contains("```{r}"));
}

#[test]
fn test_hidden_setup_code_is_only_rendered_for_quarto() {
    let root = Root::new(vec![Node::code_with_meta(
        Some("r".to_string()),
        Some("hidden".to_string()),
        "setup()",
    )]);
    let quarto = mdast_to_qmd(
        &root,
        &WriterOptions {
            quarto_code_blocks: true,
            ..Default::default()
        },
    );
    assert_eq!(quarto, "```{r}\n#| include: false\nsetup()\n```\n");

    let markdown = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(markdown.is_empty());
}

#[test]
fn test_inline_code() {
    let root = Root::new(vec![Node::paragraph(vec![
        Node::text("Use "),
        Node::inline_code("foo()"),
        Node::text(" here"),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("`foo()`"));
}

#[test]
fn test_inline_code_that_looks_executable_to_knitr_is_padded() {
    for value in ["r 1+1", "r <-  x + amount * runif(n, -1, 1)", "r\t1+1"] {
        let root = Root::new(vec![Node::paragraph(vec![Node::inline_code(value)])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert_eq!(qmd.trim(), format!("`` {value} ``"));
    }
}

#[test]
fn test_inline_code_only_guards_r_followed_by_ascii_whitespace() {
    for value in ["r", "runif(n)", "R 1+1", "python 1+1"] {
        let root = Root::new(vec![Node::paragraph(vec![Node::inline_code(value)])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert_eq!(qmd.trim(), format!("`{value}`"));
    }
}

#[test]
fn test_inline_code_with_backticks() {
    let root = Root::new(vec![Node::paragraph(vec![Node::inline_code(
        "code with ` backtick",
    )])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("`` code with ` backtick ``"));
}

#[test]
fn test_inline_code_with_double_backticks() {
    // A value containing `` `` needs a triple-backtick fence
    let root = Root::new(vec![Node::paragraph(vec![Node::inline_code("``nested``")])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("``` ``nested`` ```"));
}

#[test]
fn test_consecutive_inline_codes() {
    // Consecutive inline codes need a space between them
    // Without a space, `foo``bar` is parsed as a single code span in CommonMark
    let root = Root::new(vec![Node::paragraph(vec![
        Node::inline_code("foo"),
        Node::inline_code("bar"),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    // Should produce "`foo` `bar`" not "`foo``bar`"
    assert!(qmd.contains("`foo` `bar`"));
    assert!(!qmd.contains("`foo``bar`"));
}

#[test]
fn test_emphasis_and_strong() {
    let root = Root::new(vec![Node::paragraph(vec![
        Node::emphasis(vec![Node::text("italic")]),
        Node::text(" and "),
        Node::strong(vec![Node::text("bold")]),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("_italic_"));
    assert!(qmd.contains("**bold**"));
}

#[test]
fn test_nested_emphasis() {
    let root = Root::new(vec![Node::paragraph(vec![Node::strong(vec![
        Node::text("bold "),
        Node::emphasis(vec![Node::text("and italic")]),
    ])])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("**bold _and italic_**"));
}

#[test]
fn test_link() {
    let root = Root::new(vec![Node::paragraph(vec![Node::link(
        "https://example.com",
        vec![Node::text("Example")],
    )])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("[Example](https://example.com)"));
}

#[test]
fn test_link_destinations_are_commonmark_safe() {
    let mut lf_url = String::from(r"https://example.com/a");
    lf_url.push('\n');
    lf_url.push('b');

    let mut crlf_url = String::from(r"https://example.com/a");
    crlf_url.push('\r');
    crlf_url.push('\n');
    crlf_url.push('b');

    let mut tab_url = String::from(r"https://example.com/a");
    tab_url.push('\t');
    tab_url.push('b');
    let tab_expected = format!(r"[Example](<https://example.com/a{}b>)", '\t');

    let cases = [
        (
            String::from(r"https://example.com/path"),
            String::from(r"[Example](https://example.com/path)"),
        ),
        (
            String::from(r"https://example.com/a b"),
            String::from(r"[Example](<https://example.com/a b>)"),
        ),
        (
            lf_url,
            String::from(r"[Example](<https://example.com/a b>)"),
        ),
        (
            crlf_url,
            String::from(r"[Example](<https://example.com/a b>)"),
        ),
        (
            String::from(r"https://example.com/<a>"),
            String::from(r"[Example](<https://example.com/\<a\>>)"),
        ),
        (
            String::from(r"foo(bar"),
            String::from(r"[Example](<foo(bar>)"),
        ),
        (
            String::from(r"foo(bar)baz"),
            String::from(r"[Example](<foo(bar)baz>)"),
        ),
        (
            String::from(r"foo\<bar"),
            String::from(r"[Example](<foo\\\<bar>)"),
        ),
        (tab_url, tab_expected),
    ];

    for (url, mut expected) in cases {
        let root = Root::new(vec![Node::paragraph(vec![Node::link(
            url,
            vec![Node::text(r"Example")],
        )])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        expected.push('\n');
        assert_eq!(qmd, expected);
    }
}

#[test]
fn test_link_autolink() {
    // A link whose only child is a text equal to its URL is written as
    // an autolink, but only when the URL is a valid CommonMark
    // absolute URI (scheme prefix, no spaces)
    let cases = [
        "https://example.com",
        "x-r-help:topic",
        "topic.html",
        "https://example.com/a b",
    ];
    let root = Root::new(
        cases
            .into_iter()
            .map(|url| Node::paragraph(vec![Node::link(url, vec![Node::text(url)])]))
            .collect(),
    );
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    insta::assert_snapshot!(qmd);
}

#[test]
fn test_link_with_title() {
    let mut title = String::from(r#"Example "Site""#);
    title.push('\n');
    title.push_str(r"\ docs");
    let root = Root::new(vec![Node::paragraph(vec![Node::link_with_title(
        "https://example.com",
        title,
        vec![Node::text("Example")],
    )])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    let mut expected = String::from(r#"[Example](https://example.com "Example \"Site\" \\ docs")"#);
    expected.push('\n');
    assert_eq!(qmd, expected);
}

#[test]
fn test_image() {
    let root = Root::new(vec![Node::paragraph(vec![Node::image(
        "image.png",
        "An image",
    )])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("![An image](image.png)"));
}

#[test]
fn test_image_with_title() {
    let mut title = String::from(r#"Image "Title""#);
    title.push('\r');
    title.push('\n');
    title.push_str(r"\ docs");
    let root = Root::new(vec![Node::paragraph(vec![Node::image_with_title(
        r"image file.png",
        "An image",
        title,
    )])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    let mut expected = String::from(r#"![An image](<image file.png> "Image \"Title\" \\ docs")"#);
    expected.push('\n');
    assert_eq!(qmd, expected);
}

#[test]
fn test_unordered_list() {
    let root = Root::new(vec![Node::list(
        false,
        vec![
            Node::list_item(vec![Node::paragraph(vec![Node::text("A")])]),
            Node::list_item(vec![Node::paragraph(vec![Node::text("B")])]),
        ],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("- A"));
    assert!(qmd.contains("- B"));
}

#[test]
fn test_ordered_list() {
    let root = Root::new(vec![Node::list(
        true,
        vec![
            Node::list_item(vec![Node::paragraph(vec![Node::text("First")])]),
            Node::list_item(vec![Node::paragraph(vec![Node::text("Second")])]),
        ],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("1. First"));
    assert!(qmd.contains("2. Second"));
}

#[test]
fn test_ordered_list_custom_start() {
    let root = Root::new(vec![Node::ordered_list_from(
        5,
        vec![
            Node::list_item(vec![Node::paragraph(vec![Node::text("Five")])]),
            Node::list_item(vec![Node::paragraph(vec![Node::text("Six")])]),
        ],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("5. Five"));
    assert!(qmd.contains("6. Six"));
}

#[test]
fn test_nested_list() {
    let root = Root::new(vec![Node::list(
        false,
        vec![Node::list_item(vec![
            Node::paragraph(vec![Node::text("Parent")]),
            Node::list(
                false,
                vec![Node::list_item(vec![Node::paragraph(vec![Node::text(
                    "Child",
                )])])],
            ),
        ])],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("- Parent"));
    assert!(qmd.contains("  - Child"));
}

#[test]
fn test_loose_list_multi_paragraph() {
    let root = Root::new(vec![Node::list(
        false,
        vec![
            Node::list_item(vec![
                Node::paragraph(vec![Node::text("First paragraph")]),
                Node::paragraph(vec![Node::text("Second paragraph")]),
            ]),
            Node::list_item(vec![Node::paragraph(vec![Node::text("Another item")])]),
        ],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    // Blank line between paragraphs within the same item
    assert!(
        qmd.contains("- First paragraph\n\n  Second paragraph"),
        "got: {qmd:?}"
    );
    // Blank line between items in a loose list
    assert!(
        qmd.contains("Second paragraph\n\n- Another item"),
        "got: {qmd:?}"
    );
}

#[test]
fn test_ordered_list_loose_continuation_indent() {
    // "1. " is 3 chars; continuation lines must be indented by 3 spaces, not 2
    let root = Root::new(vec![Node::list(
        true,
        vec![Node::list_item(vec![
            Node::paragraph(vec![Node::text("First")]),
            Node::paragraph(vec![Node::text("Second")]),
        ])],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(
        qmd.contains("1. First\n\n   Second"),
        "expected 3-space continuation; got: {qmd:?}"
    );
}

#[test]
fn test_nested_list_not_double_indented() {
    // Nested item indent must be exactly item_indent (2 for "- "),
    // not 2 * item_indent (from pre-indent + recursive base_indent).
    let root = Root::new(vec![Node::list(
        false,
        vec![Node::list_item(vec![
            Node::paragraph(vec![Node::text("Parent")]),
            Node::list(
                false,
                vec![Node::list_item(vec![Node::paragraph(vec![Node::text(
                    "Child",
                )])])],
            ),
        ])],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("\n  - Child"), "got: {qmd:?}");
    assert!(
        !qmd.contains("    - Child"),
        "double-indent detected; got: {qmd:?}"
    );
}

#[test]
fn test_loose_list_block_child_continuation_indent() {
    // All lines of a multi-line block inside a list item must be indented
    // by item_indent, not just the first line.
    let root = Root::new(vec![Node::list(
        false,
        vec![Node::list_item(vec![
            Node::paragraph(vec![Node::text("Description:")]),
            Node::code(Some("r".to_string()), "x <- 1\ny <- 2"),
        ])],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(
        qmd.contains("- Description:\n\n  ```r\n  x <- 1\n  y <- 2\n  ```"),
        "code block continuation lines not indented; got: {qmd:?}"
    );
}

#[test]
fn test_definition_list_code_block_continuation_indent() {
    // A code block inside a definition description must have every line
    // (opening fence, body, closing fence) indented by 4 spaces, not just
    // the opening fence.
    let root = Root::new(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Term")]),
        Node::definition_description(vec![
            Node::paragraph(vec![Node::text("Intro")]),
            Node::code(Some("r".to_string()), "x <- 1\ny <- 2"),
        ]),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(
        qmd.contains("    ```r\n    x <- 1\n    y <- 2\n    ```"),
        "code block continuation lines not indented by 4; got: {qmd:?}"
    );
}

#[test]
fn test_definition_list_nested_list_code_block_indent() {
    // A code block inside a list item, where that list is itself inside a
    // definition description, must start on its own line (not glued to the
    // preceding item text) and have every line indented by `indent + 2`.
    let root = Root::new(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Term")]),
        Node::definition_description(vec![Node::list(
            false,
            vec![Node::list_item(vec![
                Node::paragraph(vec![Node::text("First item")]),
                Node::code(Some("r".to_string()), "code_in_list(1)\ncode_in_list(2)"),
            ])],
        )]),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(
        !qmd.contains("First item      ```"),
        "code fence glued onto preceding item text; got: {qmd:?}"
    );
    assert!(
        qmd.contains(
            "- First item\n\n      ```r\n      code_in_list(1)\n      code_in_list(2)\n      ```"
        ),
        "code block not on its own line / not indented by indent+2; got: {qmd:?}"
    );
}

#[test]
fn test_definition_list_ordered_list_code_block_indent() {
    // A code block inside an ORDERED list item, where that list is itself
    // inside a definition description, must be indented by `indent + 3`
    // (the width of "1. "), not the unordered-list width of `indent + 2`.
    let root = Root::new(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Term")]),
        Node::definition_description(vec![Node::list(
            true,
            vec![Node::list_item(vec![
                Node::paragraph(vec![Node::text("First item")]),
                Node::code(Some("r".to_string()), "code_in_list(1)\ncode_in_list(2)"),
            ])],
        )]),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(
        qmd.contains(
            "1. First item\n\n       ```r\n       code_in_list(1)\n       code_in_list(2)\n       ```"
        ),
        "code block not indented by indent+3 for ordered marker; got: {qmd:?}"
    );
}

#[test]
fn test_definition_list_ordered_list_ten_items_code_block_indent() {
    // Once the list reaches item 10, the marker becomes "10. " (4 chars),
    // so the continuation indent must widen accordingly for that item.
    let mut items: Vec<Node> = (1..=9)
        .map(|i| Node::list_item(vec![Node::paragraph(vec![Node::text(format!("Item {i}"))])]))
        .collect();
    items.push(Node::list_item(vec![
        Node::paragraph(vec![Node::text("Item 10")]),
        Node::code(Some("r".to_string()), "tenth(1)"),
    ]));
    let root = Root::new(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Term")]),
        Node::definition_description(vec![Node::list(true, items)]),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(
        qmd.contains("10. Item 10\n\n        ```r\n        tenth(1)\n        ```"),
        "code block not indented by indent+4 for '10. ' marker; got: {qmd:?}"
    );
}

#[test]
fn test_table() {
    let root = Root::new(vec![Node::table(
        vec![Some(Align::Left), Some(Align::Right)],
        vec![
            Node::table_row(vec![
                Node::table_cell(vec![Node::text("Name")]),
                Node::table_cell(vec![Node::text("Value")]),
            ]),
            Node::table_row(vec![
                Node::table_cell(vec![Node::text("foo")]),
                Node::table_cell(vec![Node::text("1")]),
            ]),
        ],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("| Name | Value |"));
    assert!(qmd.contains("|:---|---:|"));
    assert!(qmd.contains("| foo | 1 |"));
}

#[test]
fn test_table_center_align() {
    let root = Root::new(vec![Node::table(
        vec![Some(Align::Center)],
        vec![
            Node::table_row(vec![Node::table_cell(vec![Node::text("Header")])]),
            Node::table_row(vec![Node::table_cell(vec![Node::text("Data")])]),
        ],
    )]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("|:--:|"));
}

#[test]
fn test_definition_list() {
    let root = Root::new(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Term")]),
        Node::definition_description(vec![Node::paragraph(vec![Node::text("Definition")])]),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    insta::assert_snapshot!(qmd, @r###"
    Term
    :   Definition

    "###);
}

#[test]
fn test_definition_list_description_with_two_paragraphs() {
    let root = Root::new(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Term")]),
        Node::definition_description(vec![
            Node::paragraph(vec![Node::text("First paragraph")]),
            Node::paragraph(vec![Node::text("Second paragraph")]),
        ]),
    ])]);

    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    insta::assert_snapshot!(qmd, @r###"
    Term
    :   First paragraph

        Second paragraph


    "###);
}

#[test]
fn test_definition_list_description_with_math() {
    let root = Root::new(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Term")]),
        Node::definition_description(vec![Node::math("x^2 + y^2")]),
    ])]);

    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    insta::assert_snapshot!(qmd, @r###"
    Term
    :   $$
        x^2 + y^2
        $$


    "###);
}

#[test]
fn test_definition_list_description_with_nested_definition_list() {
    let nested = Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Nested term")]),
        Node::definition_description(vec![Node::paragraph(vec![Node::text(
            "Nested description",
        )])]),
    ]);
    let root = Root::new(vec![Node::definition_list(vec![
        Node::definition_term(vec![Node::text("Term")]),
        Node::definition_description(vec![nested]),
    ])]);

    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    insta::assert_snapshot!(qmd, @r###"
    Term
    :   Nested term
        :   Nested description



    "###);
}

#[test]
fn test_math() {
    let root = Root::new(vec![
        Node::paragraph(vec![Node::inline_math("x^2")]),
        Node::math("E = mc^2"),
    ]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("$x^2$"));
    assert!(qmd.contains("$$\nE = mc^2\n$$"));
}

#[test]
fn test_blockquote() {
    let root = Root::new(vec![Node::blockquote(vec![Node::paragraph(vec![
        Node::text("Quote"),
    ])])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("> Quote"));
}

#[test]
fn test_thematic_break() {
    let root = Root::new(vec![
        Node::paragraph(vec![Node::text("Before")]),
        Node::thematic_break(),
        Node::paragraph(vec![Node::text("After")]),
    ]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("Before"));
    assert!(qmd.contains("\n---\n"));
    assert!(qmd.contains("After"));
}

#[test]
fn test_line_break() {
    let root = Root::new(vec![Node::paragraph(vec![
        Node::text("Line 1"),
        Node::line_break(),
        Node::text("Line 2"),
    ])]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("Line 1  \nLine 2"));
}

#[test]
fn test_html() {
    let root = Root::new(vec![Node::html("<div>Raw HTML</div>")]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(qmd.contains("<div>Raw HTML</div>"));
}

#[test]
fn test_html_not_ending_in_newline_gets_blank_line_before_next_block() {
    // A raw HTML value that doesn't end in `\n` (e.g. a `tabled`-built
    // grid table) must still leave a full blank line before the next
    // top-level block, not just a single `\n`. Regression test for a bug
    // where `write_node`'s `Node::Html` arm left `self.at_line_start`
    // stale, so `ensure_blank_line` (which trusts that flag) between
    // root children only emitted one newline instead of two.
    let root = Root::new(vec![
        Node::html("<div>no trailing newline</div>"),
        Node::heading(2, vec![Node::text("Value")]),
    ]);
    let qmd = mdast_to_qmd(&root, &WriterOptions::default());
    assert!(
        qmd.contains("<div>no trailing newline</div>\n\n## Value"),
        "expected a blank line between HTML content and the next heading; got: {qmd:?}"
    );
}

#[test]
fn test_frontmatter() {
    let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
    let opts = WriterOptions {
        frontmatter: Some(Frontmatter {
            title: Some("My Document".to_string()),
            pagetitle: None,
            format: Some("html".to_string()),
            metadata: None,
        }),
        ..Default::default()
    };
    let qmd = mdast_to_qmd(&root, &opts);
    assert!(qmd.starts_with("---\n"));
    assert!(qmd.contains(r#"title: "My Document""#));
    assert!(qmd.contains("format: html"));
}

#[test]
fn test_frontmatter_with_pagetitle() {
    let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
    let opts = WriterOptions {
        frontmatter: Some(Frontmatter {
            title: Some("Order rows using column values".to_string()),
            pagetitle: Some("Order rows using column values — arrange".to_string()),
            format: None,
            metadata: None,
        }),
        ..Default::default()
    };
    let qmd = mdast_to_qmd(&root, &opts);
    assert!(qmd.starts_with("---\n"));
    assert!(qmd.contains(r#"title: "Order rows using column values""#));
    assert!(qmd.contains(r#"pagetitle: "Order rows using column values — arrange""#));
}

#[test]
fn test_frontmatter_escaping() {
    let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
    let opts = WriterOptions {
        frontmatter: Some(Frontmatter {
            title: Some(r#"Title with "quotes" and \backslash"#.to_string()),
            pagetitle: None,
            format: None,
            metadata: None,
        }),
        ..Default::default()
    };
    let qmd = mdast_to_qmd(&root, &opts);
    assert!(qmd.contains(r#"title: "Title with \"quotes\" and \\backslash""#));
}

#[test]
fn test_frontmatter_with_metadata() {
    let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
    let opts = WriterOptions {
        frontmatter: Some(Frontmatter {
            title: Some("My Function".to_string()),
            pagetitle: None,
            format: None,
            metadata: Some(RdMetadata {
                lifecycle: Some("deprecated".to_string()),
                aliases: vec!["my_func".to_string(), "MyFunc".to_string()],
                keywords: vec!["misc".to_string(), "internal".to_string()],
                concepts: vec!["data manipulation".to_string()],
                source_files: vec![],
            }),
        }),
        ..Default::default()
    };
    let qmd = mdast_to_qmd(&root, &opts);
    assert!(qmd.starts_with("---\n"));
    assert!(qmd.contains(r#"title: "My Function""#));
    assert!(qmd.contains("lifecycle: deprecated"));
    assert!(qmd.contains("aliases:"));
    assert!(qmd.contains(r#"  - "my_func""#));
    assert!(qmd.contains(r#"  - "MyFunc""#));
    assert!(qmd.contains("keywords:"));
    assert!(qmd.contains(r#"  - "misc""#));
    assert!(qmd.contains(r#"  - "internal""#));
    assert!(qmd.contains("concepts:"));
    assert!(qmd.contains(r#"  - "data manipulation""#));
}

#[test]
fn test_frontmatter_metadata_empty_vectors_omitted() {
    let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
    let opts = WriterOptions {
        frontmatter: Some(Frontmatter {
            title: Some("Function".to_string()),
            pagetitle: None,
            format: None,
            metadata: Some(RdMetadata {
                lifecycle: Some("stable".to_string()),
                aliases: vec![],
                keywords: vec![],
                concepts: vec![],
                source_files: vec![],
            }),
        }),
        ..Default::default()
    };
    let qmd = mdast_to_qmd(&root, &opts);
    assert!(qmd.contains("lifecycle: stable"));
    // Empty vectors should not appear
    assert!(!qmd.contains("aliases:"));
    assert!(!qmd.contains("keywords:"));
    assert!(!qmd.contains("concepts:"));
    assert!(!qmd.contains("source-files:"));
}

#[test]
fn test_frontmatter_with_source_files() {
    let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
    let opts = WriterOptions {
        frontmatter: Some(Frontmatter {
            title: Some("coord_map".to_string()),
            pagetitle: None,
            format: None,
            metadata: Some(RdMetadata {
                lifecycle: None,
                aliases: vec![],
                keywords: vec![],
                concepts: vec![],
                source_files: vec![
                    "R/coord-map.R".to_string(),
                    "R/coord-quickmap.R".to_string(),
                ],
            }),
        }),
        ..Default::default()
    };
    let qmd = mdast_to_qmd(&root, &opts);
    assert!(qmd.contains("source-files:\n"));
    assert!(qmd.contains(r#"  - "R/coord-map.R""#));
    assert!(qmd.contains(r#"  - "R/coord-quickmap.R""#));
}

// ---------------------------------------------------------------------------
// Pipe-table cell sanitization and grid-table cell rendering
//
// These moved here with the Arguments rendering itself: they describe what the
// Markdown writer does to cell content, not how Rd is parsed.
// ---------------------------------------------------------------------------

use crate::writer::arguments::convert_to_markdown_text;
use crate::writer::table_cell::sanitize_table_cell_inline_node;

#[test]
fn pipe_table_sanitizer_replaces_only_line_endings() {
    let sanitized = sanitize_table_cell_inline_node(&Node::inline_code("a  b\tc\r\nd\re\n f"));
    assert!(matches!(
        sanitized,
        Node::InlineCode(code) if code.value == "a  b\tc d e  f"
    ));
}

#[test]
fn pipe_table_sanitizer_replaces_inline_code_line_endings() {
    let sanitized = sanitize_table_cell_inline_node(&Node::inline_code("first\n second"));

    assert!(matches!(sanitized, Node::InlineCode(code) if code.value == "first  second"));
}

#[test]
fn pipe_table_sanitizer_escapes_literal_pipes_in_image_urls() {
    let sanitized = sanitize_table_cell_inline_node(&Node::image("path|name.png", "alt"));

    assert!(
        matches!(sanitized, Node::Image(image) if image.url == "path\\|name.png" && image.alt == "alt")
    );
}

#[test]
fn pipe_table_sanitizer_replaces_link_title_line_endings() {
    let sanitized = sanitize_table_cell_inline_node(&Node::link_with_title(
        "url\nvalue",
        "title\r\nvalue",
        vec![Node::text("link")],
    ));
    let markdown = mdast_to_qmd(
        &Root::new(vec![Node::paragraph(vec![sanitized])]),
        &WriterOptions::default(),
    );

    assert_eq!(markdown, "[link](<url value> \"title value\")\n");
}

#[test]
fn pipe_table_sanitizer_replaces_image_field_line_endings() {
    let sanitized = sanitize_table_cell_inline_node(&Node::image_with_title(
        "url\rvalue",
        "alt\nvalue",
        "title\r\nvalue",
    ));
    let markdown = mdast_to_qmd(
        &Root::new(vec![Node::paragraph(vec![sanitized])]),
        &WriterOptions::default(),
    );

    assert_eq!(markdown, "![alt value](<url value> \"title value\")\n");
}

#[test]
fn table_cell_serializer_formats_link_and_image_destinations() {
    let markdown = crate::writer::arguments::inline_nodes_to_markdown(&[
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
fn grid_table_list_item_second_paragraph_is_indented_under_marker() {
    // A list item's continuation content must be indented under the marker
    // column, or a grid-table cell's block-level Markdown parser stops
    // treating it as part of the same list item.
    let list = Node::list(
        false,
        vec![Node::list_item(vec![
            Node::paragraph(vec![Node::text("first paragraph")]),
            Node::paragraph(vec![Node::text("second paragraph")]),
        ])],
    );

    assert_eq!(
        convert_to_markdown_text(&[list]),
        "- first paragraph\n\n  second paragraph"
    );
}

#[test]
fn grid_table_list_item_cr_break_in_first_paragraph_is_indented_under_marker() {
    // A hard break *within the first paragraph* of a list item must also be
    // indented under the marker, not just subsequent sibling blocks.
    let list = Node::list(
        false,
        vec![Node::list_item(vec![Node::paragraph(vec![
            Node::text("first line"),
            Node::line_break(),
            Node::text("second line"),
        ])])],
    );

    assert_eq!(
        convert_to_markdown_text(&[list]),
        "- first line  \n  second line"
    );
}
