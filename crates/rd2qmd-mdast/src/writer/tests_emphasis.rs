//! Delimiter choice for emphasis: `_` by default, `*` where `_` would be
//! intraword and therefore not recognized by CommonMark.
use super::*;
use crate::mdast::*;

fn para(children: Vec<Node>) -> String {
    mdast_to_qmd(
        &Root::new(vec![Node::paragraph(children)]),
        &WriterOptions::default(),
    )
}

fn em(s: &str) -> Node {
    Node::emphasis(vec![Node::text(s)])
}

#[test]
fn intraword_both_sides_uses_star() {
    let out = para(vec![Node::text("before"), em("em"), Node::text("after")]);
    assert_eq!(out, "before*em*after\n");
}

#[test]
fn intraword_left_only_uses_star() {
    let out = para(vec![Node::text("before"), em("em"), Node::text(" after")]);
    assert_eq!(out, "before*em* after\n");
}

#[test]
fn intraword_right_only_uses_star() {
    let out = para(vec![Node::text("before "), em("em"), Node::text("after")]);
    assert_eq!(out, "before *em*after\n");
}

#[test]
fn word_boundary_keeps_underscore() {
    let out = para(vec![Node::text("a "), em("x"), Node::text(" b")]);
    assert_eq!(out, "a _x_ b\n");
    assert_eq!(para(vec![em("x")]), "_x_\n");
}

#[test]
fn strong_around_emphasis_is_unchanged() {
    let out = para(vec![Node::strong(vec![em("x")])]);
    assert_eq!(out, "**_x_**\n");
}

#[test]
fn emphasis_inside_strong_inside_word() {
    let out = para(vec![
        Node::text("a"),
        Node::strong(vec![em("x")]),
        Node::text("b"),
    ]);
    // Known limitation: this output does not parse as
    // strong(emphasis) inside a word in CommonMark (the outer `**`
    // stays literal text and only the emphasis is recognized); pinned
    // only to detect changes. To be replaced by a parsed-structure
    // assertion when inline serialization is reworked.
    assert_eq!(out, "a**_x_**b\n");
}

#[test]
fn followed_by_inline_code_keeps_underscore() {
    let out = para(vec![em("x"), Node::inline_code("y")]);
    assert_eq!(out, "_x_`y`\n");
}

#[test]
fn inside_link_text() {
    let out = para(vec![Node::link(
        "u",
        vec![Node::text("a"), em("x"), Node::text("b")],
    )]);
    assert_eq!(out, "[a*x*b](u)\n");
    let out = para(vec![Node::link("u", vec![em("x")]), Node::text("b")]);
    assert_eq!(out, "[_x_](u)b\n");
}

#[test]
fn followed_by_punctuation_keeps_underscore() {
    let out = para(vec![Node::text("a "), em("x"), Node::text(", b")]);
    assert_eq!(out, "a _x_, b\n");
}

#[test]
fn non_ascii_letters_count_as_alphanumeric() {
    let out = para(vec![Node::text("é"), em("x"), Node::text("ß")]);
    assert_eq!(out, "é*x*ß\n");
}

#[test]
fn skips_empty_text_when_looking_ahead() {
    let out = para(vec![em("x"), Node::text(""), Node::text("b")]);
    assert_eq!(out, "*x*b\n");
    // Empty text between emphasis spans does not break the run.
    let out = para(vec![
        Node::text("a"),
        em("x"),
        Node::text(""),
        em("y"),
        Node::text("b"),
    ]);
    assert_eq!(out, "a*xy*b\n");
    // Trailing empty text does not change the lookahead.
    let out = para(vec![
        Node::text("a "),
        em("x"),
        Node::text(""),
        Node::text(" b"),
    ]);
    assert_eq!(out, "a _x_ b\n");
    assert_eq!(para(vec![Node::text(""), Node::text("")]), "\n");
}

#[test]
fn in_heading_and_list_item_and_table_cell() {
    let kids = || vec![Node::text("a"), em("x"), Node::text("b")];
    let h = mdast_to_qmd(
        &Root::new(vec![Node::heading(2, kids())]),
        &WriterOptions::default(),
    );
    assert_eq!(h, "## a*x*b\n");
    let l = mdast_to_qmd(
        &Root::new(vec![Node::list(
            false,
            vec![Node::list_item(vec![Node::paragraph(kids())])],
        )]),
        &WriterOptions::default(),
    );
    assert!(l.contains("a*x*b"), "{l}");
    let t = mdast_to_qmd(
        &Root::new(vec![Node::table(
            vec![None],
            vec![
                Node::table_row(vec![Node::table_cell(vec![Node::text("h")])]),
                Node::table_row(vec![Node::table_cell(kids())]),
            ],
        )]),
        &WriterOptions::default(),
    );
    assert!(t.contains("| a*x*b |"), "{t}");
}

#[test]
fn never_adjacent_to_another_star() {
    // `**b*x***` and `***x*b**` do not parse, so `_` is used instead.
    // Known limitation: the `_` forms below do not parse as the intended
    // strong(text, emphasis) either: the `_` is intraword, so the inner
    // emphasis is lost and `_` stays literal text. Pinned only to detect
    // changes. To be replaced by a parsed-structure assertion when inline
    // serialization is reworked.
    let out = para(vec![Node::strong(vec![Node::text("b"), em("x")])]);
    assert_eq!(out, "**b_x_**\n");
    let out = para(vec![Node::strong(vec![em("x"), Node::text("b")])]);
    assert_eq!(out, "**_x_b**\n");
}

#[test]
fn nested_in_intraword_star_emphasis() {
    let inner = Node::emphasis(vec![Node::text("a"), em("x")]);
    let out = para(vec![Node::text("p"), inner, Node::text("q")]);
    // Known limitation: this output does not parse as emphasis
    // containing nested emphasis in CommonMark (everything is literal
    // text); pinned only to detect changes. To be replaced by a
    // parsed-structure assertion when inline serialization is
    // reworked.
    assert_eq!(out, "p*a_x_*q\n");
}

#[test]
fn adjacent_emphasis_is_merged() {
    let out = para(vec![Node::text("a"), em("x"), em("y"), Node::text("b")]);
    assert_eq!(out, "a*xy*b\n");
    assert_eq!(para(vec![em("x"), em("y")]), "_xy_\n");
    let out = para(vec![
        Node::text("a "),
        em("x"),
        em("y"),
        em("z"),
        Node::text(" b"),
    ]);
    assert_eq!(out, "a _xyz_ b\n");
    let out = para(vec![
        Node::text("a"),
        em("x"),
        em("y"),
        em("z"),
        Node::text("b"),
    ]);
    assert_eq!(out, "a*xyz*b\n");
}

#[test]
fn adjacent_emphasis_with_code_and_strong() {
    let out = para(vec![
        Node::text("a"),
        Node::emphasis(vec![Node::inline_code("c")]),
        Node::emphasis(vec![
            Node::text("t"),
            Node::strong(vec![Node::text("s")]),
            Node::text("u"),
        ]),
        Node::text("b"),
    ]);
    // Known limitation: this output does not parse as emphasis
    // spanning code and strong in CommonMark (the `*` delimiters stay
    // literal text and only the strong is recognized); pinned only to
    // detect changes. To be replaced by a parsed-structure assertion
    // when inline serialization is reworked.
    assert_eq!(out, "a*`c`t**s**u*b\n");
    // `*` would run into the strong delimiters, so `_` is kept.
    // Known limitation: this output does not parse as emphasis(strong) in
    // CommonMark (the `_` next to a letter and `*` cannot open or close, so
    // only the strong is recognized); pinned only to detect changes. To be
    // replaced by a parsed-structure assertion when inline serialization is
    // reworked.
    let out = para(vec![
        Node::text("a"),
        Node::emphasis(vec![Node::strong(vec![Node::text("s")])]),
        Node::text("b"),
    ]);
    assert_eq!(out, "a_**s**_b\n");
}
