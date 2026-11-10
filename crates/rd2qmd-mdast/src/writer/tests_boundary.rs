use super::*;
use crate::mdast::*;

fn render(nodes: Vec<Node>) -> String {
    mdast_to_qmd(&Root::new(nodes), &WriterOptions::default())
}

fn tbl() -> Node {
    Node::table(
        vec![None],
        vec![
            Node::table_row(vec![Node::table_cell(vec![Node::text("a")])]),
            Node::table_row(vec![Node::table_cell(vec![Node::text("b")])]),
        ],
    )
}

fn dl(desc: Vec<Node>) -> Node {
    Node::definition_list(vec![
        Node::definition_term(vec![Node::text("T")]),
        Node::definition_description(desc),
    ])
}

fn tight(child: Node) -> Node {
    Node::list(false, vec![Node::list_item(vec![child])])
}

fn para(t: &str) -> Node {
    Node::paragraph(vec![Node::text(t)])
}

fn trailing_newlines(s: &str) -> usize {
    s.len() - s.trim_end_matches('\n').len()
}

/// Pins how many trailing newlines each node leaves when rendered standalone.
#[test]
fn trailing_newlines_per_standalone_block() {
    let loose = Node::list(false, vec![Node::list_item(vec![para("a"), para("b")])]);
    // (label, node, expected output, expected trailing newline count)
    let cases: Vec<(&str, Node, &str, usize)> = vec![
        (
            "heading",
            Node::heading(1, vec![Node::text("H")]),
            "# H\n",
            1,
        ),
        ("paragraph", para("P"), "P\n", 1),
        ("thematic break", Node::thematic_break(), "---\n", 1),
        ("code", Node::code(None, "x"), "```\nx\n```\n", 1),
        ("table", tbl(), "| a |\n|----|\n| b |\n", 1),
        ("math", Node::math("x^2"), "$$\nx^2\n$$\n", 1),
        ("blockquote", Node::blockquote(vec![para("q")]), "> q\n", 1),
        // An empty table renders as an empty fragment, not a one-line block.
        ("empty table", Node::table(vec![], vec![]), "", 0),
        // Html is emitted verbatim, so the count depends on the input.
        ("html 0 newlines", Node::html("<br>"), "<br>", 0),
        ("html 1 newline", Node::html("<br>\n"), "<br>\n", 1),
        ("html 2 newlines", Node::html("<br>\n\n"), "<br>\n\n", 2),
        ("html 3 newlines", Node::html("<br>\n\n\n"), "<br>\n\n\n", 3),
        ("tight list, paragraph item", tight(para("a")), "- a\n", 1),
        // Current behavior; expected to change when the block terminator
        // contract is introduced.
        (
            "tight list, code item",
            tight(Node::code(None, "x")),
            "- ```\n  x\n  ```\n\n",
            2,
        ),
        // Current behavior (trailing blank line); expected to change when the
        // block terminator contract is introduced.
        (
            "tight list, table item",
            tight(tbl()),
            "- | a |\n  |----|\n  | b |\n\n",
            2,
        ),
        // Current behavior (trailing blank line); expected to change when the
        // block terminator contract is introduced.
        (
            "tight list, math item",
            tight(Node::math("x")),
            "- $$\n  x\n  $$\n\n",
            2,
        ),
        // Current behavior (trailing blank line); expected to change when the
        // block terminator contract is introduced.
        ("loose list", loose, "- a\n\n  b\n\n", 2),
        // Current behavior (trailing blank line); expected to change when the
        // block terminator contract is introduced.
        (
            "definition list, simplest",
            dl(vec![para("D")]),
            "T\n:   D\n\n",
            2,
        ),
        // Current behavior; expected to change when the block terminator
        // contract is introduced.
        (
            "definition list, code description",
            dl(vec![Node::code(None, "x")]),
            "T\n:   ```\n    x\n    ```\n\n\n",
            3,
        ),
        // Current behavior (trailing blank line); expected to change when the
        // block terminator contract is introduced.
        (
            "definition list, nested definition list",
            dl(vec![dl(vec![para("D")])]),
            "T\n:   T\n    :   D\n\n\n\n",
            4,
        ),
    ];
    for (label, node, expected, count) in cases {
        let out = render(vec![node]);
        assert_eq!(out, expected, "output for {label}");
        assert_eq!(
            trailing_newlines(&out),
            count,
            "trailing newlines for {label}"
        );
    }
}

/// Separators between root-level siblings.
#[test]
fn sibling_separation_at_root() {
    let cases: Vec<(&str, Vec<Node>, &str)> = vec![
        (
            "paragraph, list, paragraph",
            vec![para("A"), tight(para("x")), para("B")],
            "A\n\n- x\n\nB\n",
        ),
        (
            "list, list",
            vec![tight(para("x")), tight(para("y"))],
            "- x\n\n- y\n",
        ),
        (
            "definition list, paragraph",
            vec![dl(vec![para("D")]), para("B")],
            "T\n:   D\n\nB\n",
        ),
        (
            "paragraph, definition list",
            vec![para("B"), dl(vec![para("D")])],
            "B\n\nT\n:   D\n\n",
        ),
        (
            "definition list, definition list",
            vec![dl(vec![para("D")]), dl(vec![para("E")])],
            "T\n:   D\n\nT\n:   E\n\n",
        ),
        (
            "code, paragraph",
            vec![Node::code(None, "x"), para("B")],
            "```\nx\n```\n\nB\n",
        ),
        (
            "tight list with code item, paragraph",
            vec![tight(Node::code(None, "x")), para("B")],
            "- ```\n  x\n  ```\n\nB\n",
        ),
        (
            "html without newline, paragraph",
            vec![Node::html("<br>"), para("B")],
            "<br>\n\nB\n",
        ),
        (
            "html with newline, paragraph",
            vec![Node::html("<br>\n"), para("B")],
            "<br>\n\nB\n",
        ),
        (
            "paragraph, html with newline",
            vec![para("A"), Node::html("<br>\n")],
            "A\n\n<br>\n",
        ),
        (
            "paragraph, empty table, paragraph",
            vec![para("A"), Node::table(vec![], vec![]), para("B")],
            "A\n\nB\n",
        ),
        (
            "blockquote, paragraph",
            vec![Node::blockquote(vec![para("q")]), para("B")],
            "> q\n\nB\n",
        ),
        (
            "heading, paragraph",
            vec![Node::heading(1, vec![Node::text("H")]), para("B")],
            "# H\n\nB\n",
        ),
        (
            "thematic break, paragraph",
            vec![Node::thematic_break(), para("B")],
            "---\n\nB\n",
        ),
        (
            "table, paragraph",
            vec![tbl(), para("B")],
            "| a |\n|----|\n| b |\n\nB\n",
        ),
    ];
    for (label, nodes, expected) in cases {
        assert_eq!(render(nodes), expected, "output for {label}");
    }
}

/// Inert nodes that are only meaningful inside their parent block. When they
/// appear at the top level they are not valid blocks, and currently render
/// nothing. This is deliberately separate from the standalone block cases.
#[test]
fn inert_nodes_at_top_level_render_nothing() {
    let cases: Vec<(&str, Node)> = vec![
        ("list item", Node::list_item(vec![para("a")])),
        (
            "table row",
            Node::table_row(vec![Node::table_cell(vec![Node::text("a")])]),
        ),
        (
            "definition term",
            Node::definition_term(vec![Node::text("a")]),
        ),
        (
            "definition description",
            Node::definition_description(vec![para("a")]),
        ),
    ];
    for (label, node) in cases {
        assert_eq!(render(vec![node]), "", "output for {label}");
    }
}
