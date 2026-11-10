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
    let out = para(vec![Node::strong(vec![Node::text("b"), em("x")])]);
    assert_eq!(out, "**b_x_**\n");
    let out = para(vec![Node::strong(vec![em("x"), Node::text("b")])]);
    assert_eq!(out, "**_x_b**\n");
}

#[test]
fn nested_in_intraword_star_emphasis() {
    let inner = Node::emphasis(vec![Node::text("a"), em("x")]);
    let out = para(vec![Node::text("p"), inner, Node::text("q")]);
    assert_eq!(out, "p*a_x_*q\n");
}
