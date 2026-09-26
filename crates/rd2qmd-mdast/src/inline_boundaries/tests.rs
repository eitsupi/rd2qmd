use super::*;

#[test]
fn nested_spaces_move_without_losing_word_separators() {
    let input = vec![
        Node::text("A"),
        Node::strong(vec![Node::emphasis(vec![
            Node::text(" "),
            Node::text("foo"),
            Node::text(" "),
        ])]),
        Node::text("B"),
    ];
    let original = input.clone();
    let expected = vec![
        Node::text("A "),
        Node::strong(vec![Node::emphasis(vec![Node::text("foo")])]),
        Node::text(" B"),
    ];
    assert_eq!(normalize_inline_boundaries(&input), expected);
    assert_eq!(normalize_inline_boundaries(&expected), expected);
    assert_eq!(input, original);
}

#[test]
fn empty_and_whitespace_only_spans_emit_no_delimiters() {
    for text in ["", " ", "  \t", "\n\r", "\u{a0}\u{3000}"] {
        let input = vec![
            Node::text("A"),
            Node::emphasis(vec![Node::strong(vec![Node::text(text)])]),
            Node::text("B"),
        ];
        assert_eq!(
            normalize_inline_boundaries(&input),
            vec![Node::text(format!("A{text}B"))]
        );
    }
}

#[test]
fn commonmark_whitespace_is_not_rust_whitespace() {
    for text in ["\u{b}foo\u{b}", "\u{85}foo\u{85}", "\u{2028}foo\u{2029}"] {
        let input = vec![Node::emphasis(vec![Node::text(text)])];
        assert_eq!(normalize_inline_boundaries(&input), input);
    }
}

#[test]
fn links_keep_spaces_inside_and_leaves_are_opaque() {
    let input = vec![Node::link(
        "https://example.com",
        vec![Node::emphasis(vec![Node::text(" foo ")])],
    )];
    assert_eq!(
        normalize_inline_boundaries(&input),
        vec![Node::link(
            "https://example.com",
            vec![
                Node::text(" "),
                Node::emphasis(vec![Node::text("foo")]),
                Node::text(" ")
            ]
        )]
    );
    for leaf in [
        Node::inline_code(" x "),
        Node::inline_math(" x "),
        Node::Html(crate::Html {
            value: " <b>x</b> ".into(),
        }),
        Node::Break,
    ] {
        let input = vec![Node::strong(vec![leaf])];
        assert_eq!(normalize_inline_boundaries(&input), input);
    }
}

#[test]
fn writer_handles_direct_ast_spans_in_all_inline_containers() {
    let children = vec![
        Node::text("A"),
        Node::emphasis(vec![Node::strong(vec![Node::text(" foo ")])]),
        Node::text("B"),
    ];
    let root = crate::Root::new(vec![
        Node::paragraph(children.clone()),
        Node::heading(2, children.clone()),
        Node::list(
            false,
            vec![Node::list_item(vec![Node::paragraph(children.clone())])],
        ),
        Node::definition_list(vec![
            Node::definition_term(children.clone()),
            Node::definition_description(vec![Node::paragraph(children.clone())]),
        ]),
        Node::table(
            vec![None],
            vec![Node::table_row(vec![Node::table_cell(children)])],
        ),
    ]);
    let original = root.clone();
    let output = crate::mdast_to_qmd(&root, &crate::WriterOptions::default());
    assert_eq!(output.matches("A _**foo**_ B").count(), 6, "{output}");
    assert_eq!(root, original);
}

#[test]
fn one_sided_spaces_internal_spaces_and_breaks_are_preserved() {
    for (text, before, inner, after) in [
        (" foo", " ", "foo", ""),
        ("foo ", "", "foo", " "),
        (" foo  bar ", " ", "foo  bar", " "),
        ("\u{a0}foo\u{3000}", "\u{a0}", "foo", "\u{3000}"),
    ] {
        let input = vec![
            Node::text("A"),
            Node::emphasis(vec![Node::text(text)]),
            Node::text("B"),
        ];
        assert_eq!(
            normalize_inline_boundaries(&input),
            vec![
                Node::text(format!("A{before}")),
                Node::emphasis(vec![Node::text(inner)]),
                Node::text(format!("{after}B"))
            ]
        );
    }
    let root = crate::Root::new(vec![Node::paragraph(vec![Node::emphasis(vec![
        Node::text("a"),
        Node::Break,
        Node::text("b"),
    ])])]);
    assert_eq!(
        crate::mdast_to_qmd(&root, &crate::WriterOptions::default()),
        "_a  \nb_\n"
    );
}
