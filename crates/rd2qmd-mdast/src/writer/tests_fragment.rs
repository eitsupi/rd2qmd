//! Tests for `render_blocks_fragment`.

use super::{Frontmatter, WriterOptions, render_blocks_fragment};
use crate::mdast::Node;

#[test]
fn trims_trailing_newlines() {
    let out = render_blocks_fragment(
        &[Node::paragraph(vec![Node::text("a")])],
        &WriterOptions::default(),
    );
    assert_eq!(out, "a");
}

#[test]
fn separates_blocks_with_blank_line() {
    let out = render_blocks_fragment(
        &[
            Node::paragraph(vec![Node::text("a")]),
            Node::paragraph(vec![Node::text("b")]),
        ],
        &WriterOptions::default(),
    );
    assert_eq!(out, "a\n\nb");
}

#[test]
fn ignores_frontmatter() {
    let options = WriterOptions {
        frontmatter: Some(Frontmatter {
            title: Some("T".into()),
            ..Default::default()
        }),
        quarto_code_blocks: false,
        ..Default::default()
    };
    let out = render_blocks_fragment(&[Node::paragraph(vec![Node::text("a")])], &options);
    assert_eq!(out, "a");
}

#[test]
fn honors_quarto_code_blocks_and_empty_input() {
    let mut code = Node::code(Some("r".into()), "x\n");
    if let Node::Code(c) = &mut code {
        c.meta = Some("executable".into());
    }
    let on = WriterOptions {
        frontmatter: None,
        quarto_code_blocks: true,
        ..Default::default()
    };
    assert_eq!(
        render_blocks_fragment(&[code.clone()], &on),
        "```{r}\nx\n```"
    );
    assert_eq!(
        render_blocks_fragment(&[code], &WriterOptions::default()),
        "```r\nx\n```"
    );
    assert_eq!(render_blocks_fragment(&[], &on), "");
}

#[test]
fn hard_break_in_list_item_keeps_continuation_indented() {
    let list = Node::list(
        false,
        vec![Node::list_item(vec![Node::paragraph(vec![
            Node::text("a"),
            Node::Break,
            Node::text("b"),
        ])])],
    );
    let out = render_blocks_fragment(&[list], &WriterOptions::default());
    assert_eq!(out, "- a  \n  b");
}
