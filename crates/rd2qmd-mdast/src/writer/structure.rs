//! Test helpers that parse writer output back into a normalized block tree.
//!
//! String-equality tests pin the exact bytes, but they cannot tell whether
//! those bytes mean what was intended. These helpers parse the output with an
//! independent parser and render only the block structure: which blocks
//! exist, how they nest, and whether lists are tight or loose. Inline content
//! is flattened to plain text so each block can be identified by a short,
//! distinct label in the fixture.
//!
//! Two oracles are available:
//!
//! - [`cmark_blocks`]: pulldown-cmark with the definition list extension.
//!   Always available.
//! - [`pandoc_blocks`]: `pandoc -f markdown`, the Pandoc reader. It runs only
//!   when a `pandoc` binary is on `PATH`, and returns `None` otherwise.
//!
//! The tree only covers block structure. Inline structure (emphasis nesting,
//! escapes) is out of scope, so fixtures should use plain words and avoid
//! text that could change the block structure on its own (a leading `-`,
//! `#`, or fence).
//!
//! The two trees use the same vocabulary, so a fixture can be checked
//! against both oracles with one expected string:
//!
//! ```text
//! list tight
//!   item
//!     plain "a"
//! deflist
//!   term "T"
//!   def
//!     plain "D"
//! code "x"
//! ```
//!
//! `plain` is text directly inside a container; `para` is a paragraph.
//! A list is `tight` when its items hold `plain` text and `loose` when they
//! hold paragraphs. Definition lists carry no such label: Pandoc decides
//! `plain` or `para` per definition while pulldown-cmark decides it for the
//! whole list, so the per-definition blocks are compared instead.
//!
//! The trees cannot tell one trailing newline from two, or a blank line from
//! two. Exact-string tests remain the check for those.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::Value;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};

/// Parse `markdown` with pulldown-cmark and render its block tree.
pub(super) fn cmark_blocks(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_DEFINITION_LIST);

    let mut out = String::new();
    // Open block containers, innermost last. The tree depth is its length.
    let mut stack: Vec<Frame> = Vec::new();
    // Inline text gathered for the innermost block that owns inline content.
    let mut text: Option<String> = None;
    // Cell texts of the table row being read.
    let mut cells: Option<Vec<String>> = None;

    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(tag) => {
                // Text directly inside a list item or definition becomes a
                // `plain` block when another block starts after it.
                flush_plain(&mut out, &mut text, stack.len());
                match tag {
                    Tag::Paragraph => {
                        // pulldown-cmark wraps item text in paragraphs only
                        // in loose lists.
                        if let [.., list, item] = stack.as_mut_slice()
                            && item.kind == Kind::Item
                        {
                            list.loose = true;
                        }
                        text = Some(String::new());
                    }
                    Tag::Heading { level, .. } => {
                        line(&mut out, stack.len(), &format!("heading {}", level as u8));
                        text = Some(String::new());
                    }
                    Tag::CodeBlock(_) | Tag::HtmlBlock | Tag::TableCell => {
                        text = Some(String::new());
                    }
                    Tag::DefinitionListTitle => {
                        line(&mut out, stack.len(), "term");
                        text = Some(String::new());
                    }
                    Tag::TableHead | Tag::TableRow => cells = Some(Vec::new()),
                    Tag::BlockQuote(_) => open(&mut out, &mut stack, Kind::Other, "blockquote"),
                    Tag::List(start) => {
                        let label = match start {
                            Some(n) => format!("list ordered {n}"),
                            None => "list".to_owned(),
                        };
                        open(&mut out, &mut stack, Kind::List, &label);
                    }
                    Tag::Item => {
                        open(&mut out, &mut stack, Kind::Item, "item");
                        text = Some(String::new());
                    }
                    Tag::DefinitionList => open(&mut out, &mut stack, Kind::Other, "deflist"),
                    Tag::DefinitionListDefinition => {
                        open(&mut out, &mut stack, Kind::Other, "def");
                        text = Some(String::new());
                    }
                    Tag::Table(_) => open(&mut out, &mut stack, Kind::Other, "table"),
                    // Inline tags never change the block tree.
                    Tag::Emphasis
                    | Tag::Strong
                    | Tag::Strikethrough
                    | Tag::Superscript
                    | Tag::Subscript
                    | Tag::Link { .. }
                    | Tag::Image { .. } => {}
                    other => panic!("unexpected block {other:?} in {markdown:?}"),
                }
            }
            Event::End(end) => match end {
                TagEnd::Paragraph => block_line(&mut out, &mut text, stack.len(), "para"),
                TagEnd::CodeBlock => block_line(&mut out, &mut text, stack.len(), "code"),
                TagEnd::HtmlBlock => block_line(&mut out, &mut text, stack.len(), "html"),
                TagEnd::Heading(_) | TagEnd::DefinitionListTitle => {
                    out.pop();
                    let t = text.take().unwrap_or_default();
                    let _ = writeln!(out, " {t:?}");
                }
                TagEnd::TableCell => {
                    if let Some(cells) = &mut cells {
                        cells.push(text.take().unwrap_or_default());
                    }
                }
                TagEnd::TableHead | TagEnd::TableRow => {
                    let row = cells.take().unwrap_or_default().join(" | ");
                    line(&mut out, stack.len(), &format!("row {row:?}"));
                }
                TagEnd::Item | TagEnd::DefinitionListDefinition => {
                    flush_plain(&mut out, &mut text, stack.len());
                    stack.pop();
                }
                TagEnd::List(_) => {
                    let list = stack.pop().expect("open list");
                    let kind = if list.loose { " loose" } else { " tight" };
                    out.insert_str(list.line_end, kind);
                }
                TagEnd::BlockQuote(_) | TagEnd::DefinitionList | TagEnd::Table => {
                    stack.pop();
                }
                _ => {}
            },
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => push_text(&mut text, &t),
            Event::Code(t) => push_text(&mut text, &format!("`{t}`")),
            Event::InlineMath(t) => push_text(&mut text, &format!("${t}$")),
            Event::DisplayMath(t) => push_text(&mut text, &format!("$${t}$$")),
            Event::SoftBreak => push_text(&mut text, " "),
            Event::HardBreak => push_text(&mut text, "\\n"),
            Event::Rule => line(&mut out, stack.len(), "rule"),
            other => panic!("unexpected event {other:?} in {markdown:?}"),
        }
    }
    out
}

#[derive(PartialEq)]
enum Kind {
    List,
    Item,
    Other,
}

struct Frame {
    kind: Kind,
    /// Byte offset of the end of this container's line in the output, where
    /// a list's tightness is inserted once known. Text is only ever appended
    /// after it, so the offset stays valid.
    line_end: usize,
    loose: bool,
}

fn open(out: &mut String, stack: &mut Vec<Frame>, kind: Kind, label: &str) {
    line(out, stack.len(), label);
    stack.push(Frame {
        kind,
        line_end: out.len() - 1,
        loose: false,
    });
}

/// Write the gathered text as a block line of `kind`.
fn block_line(out: &mut String, text: &mut Option<String>, depth: usize, kind: &str) {
    let t = text.take().unwrap_or_default();
    line(out, depth, &format!("{kind} {t:?}"));
}

/// Write text gathered directly inside an item or definition as a `plain`
/// block. Such containers open a text buffer eagerly, so an empty buffer
/// means the container started with a block rather than text.
fn flush_plain(out: &mut String, text: &mut Option<String>, depth: usize) {
    if let Some(t) = text.take()
        && !t.is_empty()
    {
        line(out, depth, &format!("plain {t:?}"));
    }
}

fn push_text(text: &mut Option<String>, s: &str) {
    if let Some(t) = text {
        t.push_str(s);
    }
}

fn line(out: &mut String, depth: usize, s: &str) {
    let _ = writeln!(out, "{}{s}", "  ".repeat(depth));
}

/// Parse `markdown` with `pandoc -f markdown` and render its block tree.
///
/// Returns `None` when no `pandoc` binary is available.
pub(super) fn pandoc_blocks(markdown: &str) -> Option<String> {
    let mut child = Command::new("pandoc")
        .args(["-f", "markdown", "-t", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(markdown.as_bytes())
        .expect("write to pandoc");
    let output = child.wait_with_output().expect("wait for pandoc");
    assert!(output.status.success(), "pandoc failed on {markdown:?}");
    let doc: Value = serde_json::from_slice(&output.stdout).expect("pandoc JSON");
    let mut out = String::new();
    for block in doc["blocks"].as_array().expect("blocks array") {
        pandoc_block(&mut out, 0, block);
    }
    Some(out)
}

fn pandoc_block(out: &mut String, depth: usize, block: &Value) {
    let c = &block["c"];
    match block["t"].as_str().expect("block tag") {
        "Plain" => line(out, depth, &format!("plain {:?}", inlines(c))),
        "Para" => line(out, depth, &format!("para {:?}", inlines(c))),
        "Header" => line(
            out,
            depth,
            &format!("heading {} {:?}", c[0], inlines(&c[2])),
        ),
        "CodeBlock" => {
            let mut code = c[1].as_str().expect("code text").to_owned();
            // pulldown-cmark keeps the final newline of a fenced block;
            // Pandoc drops it. Compare the content without it.
            code.push('\n');
            line(out, depth, &format!("code {code:?}"));
        }
        "RawBlock" => line(out, depth, &format!("html {:?}", c[1].as_str().unwrap())),
        "BlockQuote" => {
            line(out, depth, "blockquote");
            blocks(out, depth + 1, c);
        }
        "HorizontalRule" => line(out, depth, "rule"),
        "BulletList" => list(out, depth, "list".to_owned(), c),
        "OrderedList" => list(out, depth, format!("list ordered {}", c[0][0]), &c[1]),
        "DefinitionList" => {
            line(out, depth, "deflist");
            for item in c.as_array().expect("definition items") {
                line(out, depth + 1, &format!("term {:?}", inlines(&item[0])));
                for def in item[1].as_array().unwrap() {
                    line(out, depth + 1, "def");
                    blocks(out, depth + 2, def);
                }
            }
        }
        "Table" => {
            line(out, depth, "table");
            let head = c[3][1].as_array().expect("head rows").iter();
            let body = c[4]
                .as_array()
                .expect("table bodies")
                .iter()
                .flat_map(|b| b[3].as_array().expect("body rows"));
            for row in head.chain(body) {
                let cells: Vec<String> = row[1]
                    .as_array()
                    .expect("cells")
                    .iter()
                    .map(|cell| {
                        cell[4]
                            .as_array()
                            .expect("cell blocks")
                            .iter()
                            .map(|b| inlines(&b["c"]))
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect();
                line(out, depth + 1, &format!("row {:?}", cells.join(" | ")));
            }
        }
        "Div" => {
            line(out, depth, "div");
            blocks(out, depth + 1, &c[1]);
        }
        other => line(out, depth, other),
    }
}

fn list(out: &mut String, depth: usize, label: String, items: &Value) {
    let items = items.as_array().expect("list items");
    line(out, depth, &format!("{label} {}", tightness(items.iter())));
    for item in items {
        line(out, depth + 1, "item");
        blocks(out, depth + 2, item);
    }
}

/// A Pandoc list is tight when its items use `Plain` rather than `Para`.
fn tightness<'v>(items: impl Iterator<Item = &'v Value>) -> &'static str {
    let loose = items.into_iter().any(|item| {
        item.as_array()
            .into_iter()
            .flatten()
            .any(|b| b["t"] == "Para")
    });
    if loose { "loose" } else { "tight" }
}

fn blocks(out: &mut String, depth: usize, value: &Value) {
    for block in value.as_array().expect("block array") {
        pandoc_block(out, depth, block);
    }
}

fn inlines(value: &Value) -> String {
    let mut s = String::new();
    for inline in value.as_array().expect("inline array") {
        let c = &inline["c"];
        match inline["t"].as_str().expect("inline tag") {
            "Str" => s.push_str(c.as_str().unwrap()),
            "Space" | "SoftBreak" => s.push(' '),
            "LineBreak" => s.push_str("\\n"),
            "Code" => {
                let _ = write!(s, "`{}`", c[1].as_str().unwrap());
            }
            "Math" => {
                let delim = if c[0]["t"] == "DisplayMath" {
                    "$$"
                } else {
                    "$"
                };
                let _ = write!(s, "{delim}{}{delim}", c[1].as_str().unwrap());
            }
            "RawInline" => s.push_str(c[1].as_str().unwrap()),
            "Emph" | "Strong" | "Strikeout" | "Superscript" | "Subscript" | "Underline"
            | "SmallCaps" => s.push_str(&inlines(c)),
            "Span" | "Link" | "Image" | "Quoted" => s.push_str(&inlines(&c[1])),
            other => {
                let _ = write!(s, "<{other}>");
            }
        }
    }
    s
}
