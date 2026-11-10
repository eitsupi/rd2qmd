//! Characterization tests pinning the exact output of the hand-rolled
//! Arguments cell renderer (`markdown_text`), which is separate from the
//! mdast writer. The snapshots record current behavior, not desired behavior.

use rd_ast::{RdDocument, RdNode, RdTag};
use rd2qmd_mdast::{Node, Root, WriterOptions, mdast_to_qmd};

use super::{BlockConversionContext, convert_arguments};
use crate::ArgumentsFormat;
use crate::convert_ast::inline::InlineConversionContext;

fn context() -> BlockConversionContext<'static> {
    BlockConversionContext {
        inline: InlineConversionContext::default(),
        prefer_ascii_math: false,
        describe_format: crate::DescribeFormat::default(),
        enclosing_heading_depth: 2,
    }
}

fn text(value: &str) -> RdNode {
    RdNode::Text(value.to_owned())
}

fn verb(value: &str) -> RdNode {
    RdNode::Verb(value.to_owned())
}

fn group(children: Vec<RdNode>) -> RdNode {
    RdNode::group(children)
}

fn tagged(tag: RdTag, children: Vec<RdNode>) -> RdNode {
    RdNode::tagged(tag, None, children)
}

fn span(tag: RdTag, value: &str) -> RdNode {
    tagged(tag, vec![text(value)])
}

fn item_marker() -> RdNode {
    tagged(RdTag::Item, vec![])
}

fn delimited_list(tag: RdTag, item_bodies: Vec<Vec<RdNode>>) -> RdNode {
    let children = item_bodies
        .into_iter()
        .flat_map(|body| std::iter::once(item_marker()).chain(body))
        .collect();
    tagged(tag, children)
}

fn described_item(label: Vec<RdNode>, body: Vec<RdNode>) -> RdNode {
    tagged(RdTag::Item, vec![group(label), group(body)])
}

fn equation(tag: RdTag, latex: &str) -> RdNode {
    tagged(tag, vec![group(vec![verb(latex)])])
}

/// Render a one-argument Arguments section in the given format and return the
/// raw output string.
fn render_arguments(format: ArgumentsFormat, description: Vec<RdNode>) -> String {
    let document = RdDocument::new(vec![tagged(
        RdTag::Arguments,
        vec![described_item(vec![text("value")], description)],
    )]);
    let arguments: Vec<_> = document.arguments_lossy().collect();
    let converted = convert_arguments(&arguments, format, &context());
    match converted.as_slice() {
        [Node::Html(html)] => html.value.clone(),
        // Pipe tables are returned as a structured table and serialized by
        // the mdast writer.
        [Node::Table(_)] => mdast_to_qmd(
            &Root::new(converted),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: true,
            },
        ),
        other => panic!("expected one raw output or table node, got {other:?}"),
    }
}

/// Render in every always-available format, concatenated with headers.
fn render_all(description: Vec<RdNode>) -> String {
    [
        ("pipe-table", ArgumentsFormat::PipeTable),
        ("list-table", ArgumentsFormat::ListTable),
        ("list", ArgumentsFormat::List),
    ]
    .into_iter()
    .map(|(name, format)| {
        format!(
            "=== {name} ===\n{}",
            render_arguments(format, description.clone())
        )
    })
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn cell_single_paragraph() {
    insta::assert_snapshot!(render_all(vec![text("Just one paragraph.")]), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | Just one paragraph. |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - Just one paragraph.

    :::

    === list ===
    - **`value`**

      Just one paragraph.
    ");
}

#[test]
fn cell_multiple_paragraphs() {
    insta::assert_snapshot!(
        render_all(vec![text("First paragraph.\n\nSecond paragraph.\n\nThird paragraph.")]),
        @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | First paragraph. <br>Second paragraph. <br>Third paragraph. |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - First paragraph.

        Second paragraph.

        Third paragraph.

    :::

    === list ===
    - **`value`**

      First paragraph.

      Second paragraph.

      Third paragraph.
    "
    );
}

#[test]
fn cell_starting_with_code_block() {
    let description = vec![
        tagged(RdTag::Preformatted, vec![verb("x <- 1\nprint(x)\n")]),
        text("Trailing prose."),
    ];
    insta::assert_snapshot!(render_all(description), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | `x <- 1 print(x) ` <br>Trailing prose. |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - 

        ```
        x <- 1
        print(x)
        ```

        Trailing prose.

    :::

    === list ===
    - **`value`**

      

      ```
      x <- 1
      print(x)
      ```

      Trailing prose.
    ");
}

#[test]
fn cell_code_block_with_blank_lines() {
    let description = vec![
        text("Intro:\n\n"),
        tagged(
            RdTag::Preformatted,
            vec![verb("a <- 1\n\nb <- 2\n\n\nc <- 3\n")],
        ),
    ];
    insta::assert_snapshot!(render_all(description), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | Intro: <br>`a <- 1  b <- 2   c <- 3 ` |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - Intro:

        ```
        a <- 1

        b <- 2


        c <- 3
        ```

    :::

    === list ===
    - **`value`**

      Intro:

      ```
      a <- 1

      b <- 2


      c <- 3
      ```
    ");
}

#[test]
fn cell_code_block_containing_fence() {
    let description = vec![
        text("Intro:\n\n"),
        tagged(RdTag::Preformatted, vec![verb("```r\nx <- 1\n```\n")]),
    ];
    insta::assert_snapshot!(render_all(description), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | Intro: <br>```` ```r x <- 1 ```  ```` |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - Intro:

        ````
        ```r
        x <- 1
        ```
        ````

    :::

    === list ===
    - **`value`**

      Intro:

      ````
      ```r
      x <- 1
      ```
      ````
    ");
}

#[test]
fn cell_nested_itemize_and_enumerate() {
    let inner = delimited_list(
        RdTag::Enumerate,
        vec![vec![text("inner one")], vec![text("inner two")]],
    );
    let outer = delimited_list(
        RdTag::Itemize,
        vec![vec![text("outer one\n\n"), inner], vec![text("outer two")]],
    );
    insta::assert_snapshot!(render_all(vec![text("Options:\n\n"), outer]), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | Options: <br>- outer one <br>1. inner one <br>2. inner two <br>- outer two |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - Options:

        - outer one

          1. inner one
          2. inner two
        - outer two

    :::

    === list ===
    - **`value`**

      Options:

      - outer one

        1. inner one
        2. inner two
      - outer two
    ");
}

#[test]
fn cell_list_only() {
    let list = delimited_list(RdTag::Enumerate, vec![vec![text("one")], vec![text("two")]]);
    insta::assert_snapshot!(render_all(vec![list]), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | 1. one <br>2. two |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - 1. one
        2. two

    :::

    === list ===
    - **`value`**

      1. one
      2. two
    ");
}

#[test]
fn cell_nested_describe() {
    let describe = tagged(
        RdTag::Describe,
        vec![
            described_item(vec![text("a")], vec![text("Use method A.")]),
            described_item(vec![text("b")], vec![text("Use method B.")]),
        ],
    );
    insta::assert_snapshot!(render_all(vec![text("Method:\n\n"), describe]), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | Method: <br>a: Use method A. <br>b: Use method B. |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - Method:

        a
        :   Use method A.

        b
        :   Use method B.

    :::

    === list ===
    - **`value`**

      Method:

      a
      :   Use method A.

      b
      :   Use method B.
    ");
}

#[test]
fn cell_empty_description() {
    insta::assert_snapshot!(render_all(vec![]), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` |  |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - 

    :::

    === list ===
    - **`value`**
    ");
}

#[test]
fn cell_inline_formatting() {
    let description = vec![
        text("Uses "),
        span(RdTag::Code, "f(x)"),
        text(", "),
        span(RdTag::Emph, "emphasis"),
        text(", "),
        span(RdTag::Strong, "strong"),
        text(", "),
        tagged(
            RdTag::Href,
            vec![
                group(vec![verb("https://example.com/a")]),
                group(vec![text("a link")]),
            ],
        ),
        text(" and "),
        span(RdTag::Code, "`tick`"),
        text("."),
    ];
    insta::assert_snapshot!(render_all(description), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | Uses `f(x)`, _emphasis_, **strong**, [a link](https://example.com/a) and `` `tick` ``. |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - Uses `f(x)`, *emphasis*, **strong**, [a link](https://example.com/a) and `` `tick` ``.

    :::

    === list ===
    - **`value`**

      Uses `f(x)`, *emphasis*, **strong**, [a link](https://example.com/a) and `` `tick` ``.
    ");
}

#[test]
fn cell_inline_and_display_math() {
    let description = vec![
        text("Inline "),
        equation(RdTag::Eqn, "x^2"),
        text(" then display:\n\n"),
        equation(RdTag::Deqn, "y = mx + b"),
    ];
    insta::assert_snapshot!(render_all(description), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | Inline $x^2$ then display: <br>$$ y = mx + b $$ |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - Inline $x^2$ then display:

        $$
        y = mx + b
        $$

    :::

    === list ===
    - **`value`**

      Inline $x^2$ then display:

      $$
      y = mx + b
      $$
    ");
}

#[test]
fn cell_list_marker_lookalikes_after_cr() {
    let cr = || tagged(RdTag::Cr, vec![]);
    let description = vec![
        text("First line."),
        cr(),
        text(" - hyphen."),
        cr(),
        text(" * asterisk."),
        cr(),
        text(" + plus."),
        cr(),
        text(" 1. ordered period."),
        cr(),
        text(" 2) ordered paren."),
        cr(),
        text(" 12 not a marker."),
    ];
    insta::assert_snapshot!(render_all(description), @r"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | First line.<br> - hyphen.<br> * asterisk.<br> + plus.<br> 1. ordered period.<br> 2) ordered paren.<br> 12 not a marker. |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - First line.  
        \- hyphen.  
        \* asterisk.  
        \+ plus.  
        1\. ordered period.  
        2\) ordered paren.  
        12 not a marker.

    :::

    === list ===
    - **`value`**

      First line.  
      \- hyphen.  
      \* asterisk.  
      \+ plus.  
      1\. ordered period.  
      2\) ordered paren.  
      12 not a marker.
    ");
}

#[test]
fn cell_leading_list_marker_lookalike_in_text() {
    // Lookalikes at the very start of a paragraph or within one text node
    // (not after a hard break).
    let description = vec![text(
        "- dash first\n\n* star first\n\n1. digit first\n\nbody\n- inner dash",
    )];
    insta::assert_snapshot!(render_all(description), @"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | - dash first <br>* star first <br>1. digit first <br>body - inner dash |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - - dash first

        * star first

        1. digit first

        body - inner dash

    :::

    === list ===
    - **`value`**

      - dash first

      * star first

      1. digit first

      body - inner dash
    ");
}

#[test]
fn cell_inline_math_pipe_and_html_break() {
    let description = vec![
        text("a | b"),
        tagged(RdTag::Cr, vec![]),
        text("second line"),
    ];
    insta::assert_snapshot!(render_all(description), @r"
    === pipe-table ===
    | Argument | Description |
    |:---|:---|
    | `value` | a \| b<br>second line |

    === list-table ===
    ::: {.list-table header-rows=1}

    - - Argument
      - Description

    - - `value`
      - a | b  
        second line

    :::

    === list ===
    - **`value`**

      a | b  
      second line
    ");
}

#[cfg(feature = "grid-table")]
#[test]
fn cell_grid_table_cases() {
    let cr = || tagged(RdTag::Cr, vec![]);
    let list = delimited_list(RdTag::Itemize, vec![vec![text("one")], vec![text("two")]]);
    let describe = tagged(
        RdTag::Describe,
        vec![described_item(vec![text("a")], vec![text("Use method A.")])],
    );
    let cases: Vec<(&str, Vec<RdNode>)> = vec![
        ("single paragraph", vec![text("Just one paragraph.")]),
        ("multiple paragraphs", vec![text("First.\n\nSecond.")]),
        (
            "code first",
            vec![tagged(
                RdTag::Preformatted,
                vec![verb("x <- 1\n\ny <- 2\n")],
            )],
        ),
        ("nested list", vec![text("Options:\n\n"), list]),
        ("describe", vec![text("Method:\n\n"), describe]),
        ("empty", vec![]),
        (
            "inline formatting",
            vec![
                span(RdTag::Code, "f(x)"),
                text(" "),
                span(RdTag::Emph, "e"),
                text(" "),
                span(RdTag::Strong, "s"),
            ],
        ),
        (
            "math",
            vec![
                equation(RdTag::Eqn, "x^2"),
                text("\n\n"),
                equation(RdTag::Deqn, "y = 1"),
            ],
        ),
        (
            "marker lookalikes",
            vec![text("A"), cr(), text(" - b"), cr(), text(" 1. c")],
        ),
    ];
    let output = cases
        .into_iter()
        .map(|(name, description)| {
            format!(
                "=== {name} ===\n{}",
                render_arguments(ArgumentsFormat::GridTable, description)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(output, @r"
    === single paragraph ===
    +----------+---------------------+
    | Argument | Description         |
    +==========+=====================+
    | `value`  | Just one paragraph. |
    +----------+---------------------+
    === multiple paragraphs ===
    +----------+-------------+
    | Argument | Description |
    +==========+=============+
    | `value`  | First.      |
    |          |             |
    |          | Second.     |
    +----------+-------------+
    === code first ===
    +----------+-------------+
    | Argument | Description |
    +==========+=============+
    | `value`  | ```         |
    |          | x <- 1      |
    |          |             |
    |          | y <- 2      |
    |          |             |
    |          | ```         |
    +----------+-------------+
    === nested list ===
    +----------+-------------+
    | Argument | Description |
    +==========+=============+
    | `value`  | Options:    |
    |          |             |
    |          | - one       |
    |          | - two       |
    +----------+-------------+
    === describe ===
    +----------+-------------------+
    | Argument | Description       |
    +==========+===================+
    | `value`  | Method:           |
    |          |                   |
    |          | a                 |
    |          | :   Use method A. |
    +----------+-------------------+
    === empty ===
    +----------+-------------+
    | Argument | Description |
    +==========+=============+
    | `value`  |             |
    +----------+-------------+
    === inline formatting ===
    +----------+------------------+
    | Argument | Description      |
    +==========+==================+
    | `value`  | `f(x)` *e* **s** |
    +----------+------------------+
    === math ===
    +----------+-------------+
    | Argument | Description |
    +==========+=============+
    | `value`  | $x^2$       |
    |          |             |
    |          | $$          |
    |          | y = 1       |
    |          | $$          |
    +----------+-------------+
    === marker lookalikes ===
    +----------+-------------+
    | Argument | Description |
    +==========+=============+
    | `value`  | A           |
    |          | \- b        |
    |          | 1\. c       |
    +----------+-------------+
    ");
}
