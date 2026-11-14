//! Block structure of writer output, checked by parsing it back.
//!
//! These complement the exact-string tests: a change in newlines that keeps
//! the tree is invisible here, and a change that moves a block shows up even
//! when the new string looks plausible. The Pandoc oracle runs only when a
//! `pandoc` binary is on `PATH`; set `RD2QMD_REQUIRE_PANDOC` to make its
//! absence a failure.
use super::structure::{cmark_blocks, pandoc_blocks};
use super::*;
use crate::mdast::*;

fn render(nodes: Vec<Node>) -> String {
    mdast_to_qmd(&Root::new(nodes), &WriterOptions::default())
}

fn para(t: &str) -> Node {
    Node::paragraph(vec![Node::text(t)])
}

fn code(t: &str) -> Node {
    Node::code(None, t)
}

fn items(children: Vec<Vec<Node>>) -> Vec<Node> {
    children.into_iter().map(Node::list_item).collect()
}

fn bullets(children: Vec<Vec<Node>>) -> Node {
    Node::list(false, items(children))
}

fn term(t: &str) -> Node {
    Node::definition_term(vec![Node::text(t)])
}

fn def(children: Vec<Node>) -> Node {
    Node::definition_description(children)
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

/// Root-level block sequences, with the block tree pulldown-cmark reads from
/// the output and, when it differs, the tree Pandoc reads.
#[allow(clippy::type_complexity)]
fn cases() -> Vec<(&'static str, Vec<Node>, &'static str, Option<&'static str>)> {
    vec![
        (
            "root siblings",
            vec![para("A"), bullets(vec![vec![para("x")]]), para("B")],
            r#"
para "A"
list tight
  item
    plain "x"
para "B"
"#,
            None,
        ),
        // Known bug: two adjacent lists of the same kind merge into one loose list.
        (
            "root list, list",
            vec![
                bullets(vec![vec![para("x")]]),
                bullets(vec![vec![para("y")]]),
            ],
            r#"
list loose
  item
    para "x"
  item
    para "y"
"#,
            None,
        ),
        (
            "tight list of code items",
            vec![bullets(vec![vec![code("x")], vec![code("y")]]), para("B")],
            r#"
list tight
  item
    code "x\n"
  item
    code "y\n"
para "B"
"#,
            None,
        ),
        (
            "tight list of table items",
            vec![bullets(vec![vec![tbl()], vec![para("c")]]), para("B")],
            r#"
list tight
  item
    table
      row ["a"]
      row ["b"]
  item
    plain "c"
para "B"
"#,
            None,
        ),
        (
            "loose list",
            vec![
                bullets(vec![vec![para("a"), para("b")], vec![para("c")]]),
                para("B"),
            ],
            r#"
list loose
  item
    para "a"
    para "b"
  item
    para "c"
para "B"
"#,
            None,
        ),
        (
            "loose list with code",
            vec![
                bullets(vec![vec![para("a"), code("x")], vec![para("c")]]),
                para("B"),
            ],
            r#"
list loose
  item
    para "a"
    code "x\n"
  item
    para "c"
para "B"
"#,
            None,
        ),
        (
            "nested tight list",
            vec![
                bullets(vec![
                    vec![para("a"), bullets(vec![vec![para("b")]])],
                    vec![para("c")],
                ]),
                para("B"),
            ],
            r#"
list loose
  item
    para "a"
    list tight
      item
        plain "b"
  item
    para "c"
para "B"
"#,
            None,
        ),
        (
            "ordered wide numbers",
            vec![Node::ordered_list_from(
                9,
                items(vec![
                    vec![para("a")],
                    vec![para("b"), para("c")],
                    vec![para("d")],
                ]),
            )],
            r#"
list ordered 9 loose
  item
    para "a"
  item
    para "b"
    para "c"
  item
    para "d"
"#,
            None,
        ),
        (
            "dl simple",
            vec![
                Node::definition_list(vec![term("T"), def(vec![para("D")])]),
                para("B"),
            ],
            r#"
deflist
  term "T"
  def
    plain "D"
para "B"
"#,
            None,
        ),
        (
            "dl code",
            vec![
                Node::definition_list(vec![term("T"), def(vec![code("x")])]),
                para("B"),
            ],
            r#"
deflist
  term "T"
  def
    code "x\n"
para "B"
"#,
            None,
        ),
        (
            "dl nested dl",
            vec![
                Node::definition_list(vec![
                    term("T"),
                    def(vec![Node::definition_list(vec![
                        term("U"),
                        def(vec![para("D")]),
                    ])]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T"
  def
    deflist
      term "U"
      def
        plain "D"
para "B"
"#,
            None,
        ),
        (
            "dl list only",
            vec![
                Node::definition_list(vec![
                    term("T"),
                    def(vec![bullets(vec![vec![para("a")], vec![para("b")]])]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T"
  def
    list tight
      item
        plain "a"
      item
        plain "b"
para "B"
"#,
            None,
        ),
        (
            "dl ordered list with wide numbers only",
            vec![
                Node::definition_list(vec![
                    term("T"),
                    def(vec![Node::ordered_list_from(
                        9,
                        items(vec![vec![para("a")], vec![para("b")]]),
                    )]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T"
  def
    list ordered 9 tight
      item
        plain "a"
      item
        plain "b"
para "B"
"#,
            None,
        ),
        (
            "dl list then paragraph",
            vec![
                Node::definition_list(vec![
                    term("T"),
                    def(vec![bullets(vec![vec![para("a")]]), para("P")]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T"
  def
    list tight
      item
        plain "a"
    para "P"
para "B"
"#,
            None,
        ),
        (
            "dl para then list",
            vec![
                Node::definition_list(vec![
                    term("T"),
                    def(vec![
                        para("D"),
                        bullets(vec![vec![para("a")], vec![para("b")]]),
                    ]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T"
  def
    para "D"
    list tight
      item
        plain "a"
      item
        plain "b"
para "B"
"#,
            None,
        ),
        (
            "dl two terms",
            vec![
                Node::definition_list(vec![
                    term("T1"),
                    def(vec![para("D1")]),
                    term("T2"),
                    def(vec![para("D2")]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T1"
  def
    plain "D1"
  term "T2"
  def
    plain "D2"
para "B"
"#,
            None,
        ),
        // pulldown-cmark decides paragraphs for the whole definition list, Pandoc
        // per definition.
        (
            "dl two terms, code then term",
            vec![
                Node::definition_list(vec![
                    term("T1"),
                    def(vec![para("D1"), code("x")]),
                    term("T2"),
                    def(vec![para("D2")]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T1"
  def
    para "D1"
    code "x\n"
  term "T2"
  def
    para "D2"
para "B"
"#,
            Some(
                r#"
deflist
  term "T1"
  def
    para "D1"
    code "x\n"
  term "T2"
  def
    plain "D2"
para "B"
"#,
            ),
        ),
        // pulldown-cmark decides paragraphs for the whole definition list, Pandoc
        // per definition.
        (
            "dl two terms, list then term",
            vec![
                Node::definition_list(vec![
                    term("T1"),
                    def(vec![para("D1"), bullets(vec![vec![para("a")]])]),
                    term("T2"),
                    def(vec![para("D2")]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T1"
  def
    para "D1"
    list tight
      item
        plain "a"
  term "T2"
  def
    para "D2"
para "B"
"#,
            Some(
                r#"
deflist
  term "T1"
  def
    para "D1"
    list tight
      item
        plain "a"
  term "T2"
  def
    plain "D2"
para "B"
"#,
            ),
        ),
        // pulldown-cmark decides paragraphs for the whole definition list, Pandoc
        // per definition.
        (
            "dl two terms, nested dl then term",
            vec![
                Node::definition_list(vec![
                    term("T1"),
                    def(vec![
                        para("D1"),
                        Node::definition_list(vec![term("U"), def(vec![para("E")])]),
                    ]),
                    term("T2"),
                    def(vec![para("D2")]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T1"
  def
    para "D1"
    deflist
      term "U"
      def
        plain "E"
  term "T2"
  def
    para "D2"
para "B"
"#,
            Some(
                r#"
deflist
  term "T1"
  def
    para "D1"
    deflist
      term "U"
      def
        plain "E"
  term "T2"
  def
    plain "D2"
para "B"
"#,
            ),
        ),
        (
            "dl two descriptions",
            vec![
                Node::definition_list(vec![
                    term("T"),
                    def(vec![para("D1")]),
                    def(vec![para("D2")]),
                ]),
                para("B"),
            ],
            r#"
deflist
  term "T"
  def
    plain "D1"
  def
    plain "D2"
para "B"
"#,
            None,
        ),
        (
            "dl in list item",
            vec![
                bullets(vec![
                    vec![Node::definition_list(vec![term("T"), def(vec![para("D")])])],
                    vec![para("c")],
                ]),
                para("B"),
            ],
            r#"
list tight
  item
    deflist
      term "T"
      def
        plain "D"
  item
    plain "c"
para "B"
"#,
            None,
        ),
        (
            "table, para",
            vec![tbl(), para("B")],
            r#"
table
  row ["a"]
  row ["b"]
para "B"
"#,
            None,
        ),
        (
            "blockquote, para",
            vec![Node::blockquote(vec![para("q")]), para("B")],
            r#"
blockquote
  para "q"
para "B"
"#,
            None,
        ),
    ]
}

#[test]
fn block_structure_of_writer_output() {
    let require_pandoc = std::env::var_os("RD2QMD_REQUIRE_PANDOC").is_some();
    // Report every mismatch at once: a writer change usually moves several
    // cases together.
    let mut failures = Vec::new();
    for (label, nodes, cmark, pandoc) in cases() {
        let md = render(nodes);
        let cmark = cmark.trim_start_matches('\n');
        let tree = cmark_blocks(&md);
        if tree != cmark {
            failures.push(format!("pulldown-cmark tree for {label}: {md:?}\n{tree}"));
        }
        match pandoc_blocks(&md) {
            Some(tree) => {
                let pandoc = pandoc.map_or(cmark, |p| p.trim_start_matches('\n'));
                if tree != pandoc {
                    failures.push(format!("Pandoc tree for {label}: {md:?}\n{tree}"));
                }
            }
            None => assert!(!require_pandoc, "pandoc is required but not found"),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
