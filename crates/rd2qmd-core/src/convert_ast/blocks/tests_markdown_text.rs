//! Direct tests of the hand-rolled cell renderer in `markdown_text`; delete
//! this file together with that renderer once cells go through the writer.

use rd2qmd_mdast::Node;

use super::markdown_text::{
    inline_nodes_to_markdown, render_block_content, render_list_table_cell,
};

fn para(children: Vec<Node>) -> Node {
    Node::paragraph(children)
}

#[test]
fn render_block_content_cases() {
    let list_item = |children: Vec<Node>| Node::list_item(children);
    let cases: Vec<(&str, Vec<Node>, u8)> = vec![
        (
            "paragraphs indent 4",
            vec![para(vec![Node::text("a\nb")]), para(vec![Node::text("c")])],
            4,
        ),
        (
            "paragraphs indent 2",
            vec![para(vec![Node::text("a")]), para(vec![Node::text("c")])],
            2,
        ),
        (
            "empty paragraph skipped",
            vec![para(vec![Node::text("  ")]), para(vec![Node::text("x")])],
            4,
        ),
        (
            "code first",
            vec![Node::code(Some("r".into()), "x\n\ny\n")],
            4,
        ),
        (
            "code after paragraph",
            vec![para(vec![Node::text("p")]), Node::code(None, "x")],
            4,
        ),
        (
            "tight list",
            vec![Node::list(
                false,
                vec![
                    list_item(vec![para(vec![Node::text("a")])]),
                    list_item(vec![para(vec![Node::text("b")])]),
                ],
            )],
            4,
        ),
        (
            "ordered list after paragraph",
            vec![
                para(vec![Node::text("p")]),
                Node::list(
                    true,
                    vec![
                        list_item(vec![para(vec![Node::text("a")])]),
                        list_item(vec![para(vec![Node::text("b")])]),
                    ],
                ),
            ],
            4,
        ),
        (
            "list item with code child",
            vec![Node::list(
                false,
                vec![list_item(vec![
                    para(vec![Node::text("a")]),
                    Node::code(None, "x\n\ny"),
                ])],
            )],
            4,
        ),
        ("math", vec![Node::math("x^2")], 4),
        (
            "heading",
            vec![
                para(vec![Node::text("p")]),
                Node::heading(3, vec![Node::text("H")]),
            ],
            4,
        ),
        ("empty", vec![], 4),
    ];
    let output = cases
        .into_iter()
        .map(|(name, nodes, indent)| {
            format!("=== {name} ===\n{:?}", render_block_content(&nodes, indent))
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(output, @r#"
    === paragraphs indent 4 ===
    "a\n    b\n\n    c"
    === paragraphs indent 2 ===
    "a\n\n  c"
    === empty paragraph skipped ===
    "x"
    === code first ===
    "\n\n    ```r\n    x\n\n    y\n    ```"
    === code after paragraph ===
    "p\n\n    ```\n    x\n    ```"
    === tight list ===
    "- a\n    - b"
    === ordered list after paragraph ===
    "p\n\n    1. a\n    2. b"
    === list item with code child ===
    "- a\n\n      ```\n      x\n\n      y\n      ```"
    === math ===
    "\n\n    $$\n    x^2\n    $$"
    === heading ===
    "p\n\n    ### H"
    === empty ===
    ""
    "#);
}

#[test]
fn render_list_table_cell_uses_four_space_indent() {
    let nodes = vec![para(vec![Node::text("a")]), para(vec![Node::text("b")])];
    assert_eq!(
        render_list_table_cell(&nodes),
        render_block_content(&nodes, 4)
    );
}

#[test]
fn inline_nodes_to_markdown_cases() {
    let cases: Vec<(&str, Vec<Node>)> = vec![
        ("text", vec![Node::text("plain")]),
        ("inline code", vec![Node::inline_code("f(x)")]),
        ("inline code with backtick", vec![Node::inline_code("a`b")]),
        (
            "adjacent inline code",
            vec![Node::text("a`"), Node::inline_code("b")],
        ),
        ("emphasis", vec![Node::emphasis(vec![Node::text("e")])]),
        ("strong", vec![Node::strong(vec![Node::text("s")])]),
        (
            "nested",
            vec![Node::strong(vec![
                Node::text("a "),
                Node::emphasis(vec![Node::text("b")]),
                Node::text(" c"),
            ])],
        ),
        (
            "link",
            vec![Node::link("https://example.com", vec![Node::text("l")])],
        ),
        (
            "link with title",
            vec![Node::link_with_title(
                "https://example.com",
                "t\"q",
                vec![Node::text("l")],
            )],
        ),
        (
            "link with space",
            vec![Node::link("https://example.com/a b", vec![Node::text("l")])],
        ),
        ("image", vec![Node::image("a.png", "alt")]),
        ("inline math", vec![Node::inline_math("x^2")]),
        (
            "break",
            vec![Node::text("a"), Node::line_break(), Node::text("b")],
        ),
        ("html", vec![Node::html("<b>")]),
        (
            "emphasis edge whitespace",
            vec![Node::emphasis(vec![Node::text(" e ")])],
        ),
    ];
    let output = cases
        .into_iter()
        .map(|(name, nodes)| format!("=== {name} ===\n{:?}", inline_nodes_to_markdown(&nodes)))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(output, @r#"
    === text ===
    "plain"
    === inline code ===
    "`f(x)`"
    === inline code with backtick ===
    "`` a`b ``"
    === adjacent inline code ===
    "a` `b`"
    === emphasis ===
    "*e*"
    === strong ===
    "**s**"
    === nested ===
    "**a *b* c**"
    === link ===
    "[l](https://example.com)"
    === link with title ===
    "[l](https://example.com \"t\\\"q\")"
    === link with space ===
    "[l](<https://example.com/a b>)"
    === image ===
    "![alt](a.png)"
    === inline math ===
    "$x^2$"
    === break ===
    "a  \nb"
    === html ===
    "<b>"
    === emphasis edge whitespace ===
    " *e* "
    "#);
}
