//! Definition lists written as Quarto `::: {.definition-list}` divs.
use super::*;
use crate::mdast::*;

fn render(nodes: Vec<Node>) -> String {
    mdast_to_qmd(
        &Root::new(nodes),
        &WriterOptions {
            definition_list_syntax: DefinitionListSyntax::QuartoDiv,
            ..Default::default()
        },
    )
}

fn para(t: &str) -> Node {
    Node::paragraph(vec![Node::text(t)])
}

fn term(t: &str) -> Node {
    Node::definition_term(vec![Node::text(t)])
}

fn def(children: Vec<Node>) -> Node {
    Node::definition_description(children)
}

#[test]
fn quarto_definition_lists() {
    let cases: Vec<(&str, Vec<Node>, &str)> = vec![
        (
            "two terms, then paragraph",
            vec![
                Node::definition_list(vec![
                    term("T1"),
                    def(vec![para("D1")]),
                    term("T2"),
                    def(vec![para("D2")]),
                ]),
                para("B"),
            ],
            "::: {.definition-list}\n\n- T1\n\n  - D1\n\n- T2\n\n  - D2\n\n:::\n\nB\n",
        ),
        (
            "multi-block description",
            vec![Node::definition_list(vec![
                term("T"),
                def(vec![para("P1"), Node::code(None, "x"), para("P2")]),
            ])],
            "::: {.definition-list}\n\n- T\n\n  - P1\n\n    ```\n    x\n    ```\n\n    P2\n\n:::\n",
        ),
        (
            "description holding only a list",
            vec![Node::definition_list(vec![
                term("T"),
                def(vec![Node::list(
                    false,
                    vec![
                        Node::list_item(vec![para("a")]),
                        Node::list_item(vec![para("b")]),
                    ],
                )]),
            ])],
            "::: {.definition-list}\n\n- T\n\n  - - a\n    - b\n\n:::\n",
        ),
        // Quarto needs both the term and the description block to recognize
        // an item, so an empty description gets a zero-width placeholder.
        (
            "empty description",
            vec![Node::definition_list(vec![term("T"), def(vec![])])],
            "::: {.definition-list}\n\n- T\n\n  - &#8203;\n\n:::\n",
        ),
        (
            "term without description",
            vec![Node::definition_list(vec![term("T")])],
            "::: {.definition-list}\n\n- T\n\n  - &#8203;\n\n:::\n",
        ),
        // Each description is its own nested item, which Quarto and q2
        // render as separate `<dd>` elements.
        (
            "two descriptions",
            vec![Node::definition_list(vec![
                term("T"),
                def(vec![para("D1")]),
                def(vec![para("D2")]),
            ])],
            "::: {.definition-list}\n\n- T\n\n  - D1\n  - D2\n\n:::\n",
        ),
        (
            "empty description among others",
            vec![Node::definition_list(vec![
                term("T"),
                def(vec![]),
                def(vec![para("D2"), para("D3")]),
            ])],
            "::: {.definition-list}\n\n- T\n\n  - &#8203;\n\n  - D2\n\n    D3\n\n:::\n",
        ),
        (
            "nested definition list",
            vec![Node::definition_list(vec![
                term("T"),
                def(vec![
                    para("D"),
                    Node::definition_list(vec![term("U"), def(vec![para("E")])]),
                ]),
            ])],
            "::: {.definition-list}\n\n- T\n\n  - D\n\n    ::: {.definition-list}\n\n    - U\n\n      - E\n\n    :::\n\n:::\n",
        ),
        (
            "inside a list item",
            vec![Node::list(
                false,
                vec![
                    Node::list_item(vec![Node::definition_list(vec![
                        term("T"),
                        def(vec![para("D")]),
                    ])]),
                    Node::list_item(vec![para("c")]),
                ],
            )],
            "- ::: {.definition-list}\n\n  - T\n\n    - D\n\n  :::\n- c\n",
        ),
        (
            "without terms writes nothing",
            vec![para("A"), Node::definition_list(vec![]), para("B")],
            "A\n\nB\n",
        ),
    ];
    for (label, nodes, expected) in cases {
        assert_eq!(render(nodes), expected, "output for {label}");
    }
}
