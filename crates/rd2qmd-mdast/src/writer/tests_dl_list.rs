//! Pins how lists nested inside definition descriptions are rendered. These
//! differ from top-level lists: no list inside a description is ever loose.
//! Some outputs (e.g. the 8-space first item) are quirks pinned as-is.
use super::*;
use crate::mdast::*;

fn render(nodes: Vec<Node>) -> String {
    mdast_to_qmd(&Root::new(nodes), &WriterOptions::default())
}

fn dl(desc: Vec<Node>) -> Node {
    Node::definition_list(vec![
        Node::definition_term(vec![Node::text("T")]),
        Node::definition_description(desc),
    ])
}

fn para(t: &str) -> Node {
    Node::paragraph(vec![Node::text(t)])
}

fn item(children: Vec<Node>) -> Node {
    Node::list_item(children)
}

fn bullets(items: Vec<Node>) -> Node {
    Node::list(false, items)
}

#[test]
fn lists_inside_definition_descriptions() {
    let cases: Vec<(&str, Vec<Node>, &str)> = vec![
        (
            "tight list",
            vec![bullets(vec![item(vec![para("a")]), item(vec![para("b")])])],
            "T\n:       - a\n    - b\n",
        ),
        (
            "multi-block item stays tight",
            vec![bullets(vec![
                item(vec![para("a"), para("b")]),
                item(vec![para("c")]),
            ])],
            "T\n:       - a\n\n      b\n    - c\n",
        ),
        (
            "ordered list with wide numbers",
            vec![Node::ordered_list_from(
                9,
                vec![
                    item(vec![para("a")]),
                    item(vec![para("b"), para("c")]),
                    item(vec![para("d")]),
                ],
            )],
            "T\n:       9. a\n    10. b\n\n        c\n    11. d\n",
        ),
        (
            "nested list",
            vec![bullets(vec![
                item(vec![para("a"), bullets(vec![item(vec![para("b")])])]),
                item(vec![para("c")]),
            ])],
            "T\n:       - a\n\n      - b\n    - c\n",
        ),
        (
            "nested list in wide ordered item",
            vec![Node::ordered_list_from(
                10,
                vec![item(vec![
                    para("a"),
                    bullets(vec![item(vec![para("b")]), item(vec![para("c")])]),
                ])],
            )],
            "T\n:       10. a\n\n        - b\n        - c\n",
        ),
        (
            "item with code",
            vec![bullets(vec![
                item(vec![para("a"), Node::code(None, "x\ny")]),
                item(vec![para("b")]),
            ])],
            "T\n:       - a\n\n      ```\n      x\n      y\n      ```\n    - b\n",
        ),
        (
            "item starting with code",
            vec![bullets(vec![
                item(vec![Node::code(None, "x")]),
                item(vec![para("b")]),
            ])],
            "T\n:       - \n      ```\n      x\n      ```\n    - b\n",
        ),
        (
            "paragraph then list",
            vec![para("P"), bullets(vec![item(vec![para("a")])])],
            "T\n:   P\n\n    - a\n",
        ),
        (
            "list then paragraph",
            vec![bullets(vec![item(vec![para("a")])]), para("P")],
            "T\n:       - a\n\n    P\n",
        ),
        (
            "list then code",
            vec![bullets(vec![item(vec![para("a")])]), Node::code(None, "x")],
            "T\n:       - a\n\n    ```\n    x\n    ```\n",
        ),
        (
            "two lists",
            vec![
                bullets(vec![item(vec![para("a")])]),
                Node::list(true, vec![item(vec![para("b")])]),
            ],
            "T\n:       - a\n\n    1. b\n",
        ),
    ];
    for (label, desc, expected) in cases {
        assert_eq!(render(vec![dl(desc)]), expected, "output for {label}");
    }
}
