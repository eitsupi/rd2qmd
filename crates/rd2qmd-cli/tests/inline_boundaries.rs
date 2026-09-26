//! Boundary whitespace must preserve both words and inline semantics.
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use rd2qmd_core::{ArgumentsFormat, DescribeFormat, RdConvertOptions};

const INPUT: &str = r"\name{spaces}\title{Spaces}
\description{
A\emph{ foo }B.

A\strong{ bar }B.

A\strong{\emph{ nested }}B.

A\href{https://example.com}{\emph{ linked }}B.

A\emph{ \code{x | y} }B.

A\emph{}\strong{ }B.

\describe{\item{\emph{ label }}{Label body.}
\item{\strong{\emph{ }}}{Empty label body.}}
}
\arguments{\item{x}{A\emph{ foo }B.

A\strong{\emph{ nested }}B.
\describe{\item{\emph{ term }}{Argument body.}\item{\strong{\emph{ }}}{Empty argument label body.}}
}}";

#[test]
fn whitespace_across_description_and_argument_formats() {
    let parsed = rd2qmd_source::parse(INPUT).unwrap();
    for arguments_format in [
        ArgumentsFormat::List,
        ArgumentsFormat::PipeTable,
        ArgumentsFormat::ListTable,
        ArgumentsFormat::GridTable,
    ] {
        for describe_format in [
            DescribeFormat::DefinitionList,
            DescribeFormat::List,
            DescribeFormat::Headings,
        ] {
            let output = rd2qmd_core::convert_rd_document(
                parsed.document(),
                &RdConvertOptions {
                    arguments_format: arguments_format.clone(),
                    describe_format,
                    ..Default::default()
                },
            );
            {
                // Show preserved end-of-line spaces explicitly without trailing
                // whitespace in the snapshot file itself.
                let visible_spaces = output
                    .lines()
                    .map(|line| {
                        let body = line.trim_end_matches(' ');
                        format!("{body}{}", "␠".repeat(line.len() - body.len()))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                insta::assert_snapshot!(
                    format!("{arguments_format:?}_{describe_format:?}"),
                    visible_spaces
                );
            }
        }
    }
}

/// Snapshot the complete parse event stream, including empty containers, so
/// text cannot silently move out of a list item while keeping a test green.
fn structure(markdown: &str, options: Options) -> String {
    let mut output = String::new();
    let mut depth = 0;
    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(tag) => {
                let label = match tag {
                    Tag::Link {
                        link_type,
                        dest_url,
                        title,
                        id,
                    } => format!(
                        "Link({link_type:?}, {:?}, {:?}, {:?})",
                        dest_url.as_ref(),
                        title.as_ref(),
                        id.as_ref()
                    ),
                    Tag::CodeBlock(CodeBlockKind::Fenced(lang)) => {
                        format!("CodeBlock({:?})", lang.as_ref())
                    }
                    other => format!("{other:?}"),
                };
                output.push_str(&format!("{}+ {label}\n", "  ".repeat(depth)));
                depth += 1;
            }
            Event::End(tag) => {
                depth -= 1;
                output.push_str(&format!("{}- {tag:?}\n", "  ".repeat(depth)));
            }
            Event::Text(text) => output.push_str(&format!(
                "{}Text({:?})\n",
                "  ".repeat(depth),
                text.as_ref()
            )),
            Event::Code(text) => output.push_str(&format!(
                "{}Code({:?})\n",
                "  ".repeat(depth),
                text.as_ref()
            )),
            leaf => output.push_str(&format!("{}{leaf:?}\n", "  ".repeat(depth))),
        }
    }
    assert_eq!(depth, 0);
    output
}

fn convert(rd: &str, format: DescribeFormat) -> String {
    let parsed = rd2qmd_source::parse(rd).unwrap();
    rd2qmd_core::convert_rd_document(
        parsed.document(),
        &RdConvertOptions {
            describe_format: format,
            arguments_format: ArgumentsFormat::List,
            ..Default::default()
        },
    )
}

#[test]
fn padded_inline_semantics() {
    let output = convert(
        r"\name{spans}\title{Spans}\description{
A\emph{ foo }B.

A\strong{ bar }B.

A\strong{\emph{ nested }}B.

A\href{https://example.com}{\emph{ linked }}B.

A\emph{ \code{x | y} }B.

A\emph{}\strong{ }B.
}",
        DescribeFormat::List,
    );
    insta::assert_snapshot!(structure(&output, Options::empty()), @r#"
    + Heading { level: H1, id: None, classes: [], attrs: [] }
      Text("Spans")
    - Heading(H1)
    + Heading { level: H2, id: None, classes: [], attrs: [] }
      Text("Description")
    - Heading(H2)
    + Paragraph
      Text("A ")
      + Emphasis
        Text("foo")
      - Emphasis
      Text(" B.")
    - Paragraph
    + Paragraph
      Text("A ")
      + Strong
        Text("bar")
      - Strong
      Text(" B.")
    - Paragraph
    + Paragraph
      Text("A ")
      + Strong
        + Emphasis
          Text("nested")
        - Emphasis
      - Strong
      Text(" B.")
    - Paragraph
    + Paragraph
      Text("A")
      + Link(Inline, "https://example.com", "", "")
        Text(" ")
        + Emphasis
          Text("linked")
        - Emphasis
        Text(" ")
      - Link
      Text("B.")
    - Paragraph
    + Paragraph
      Text("A ")
      + Emphasis
        Code("x | y")
      - Emphasis
      Text(" B.")
    - Paragraph
    + Paragraph
      Text("A B.")
    - Paragraph
    "#);
}

const EMPTY_LABELS: &str = r"\name{labels}\title{Labels}
\description{\describe{
\item{first}{First body.}
\item{\strong{\emph{ }}}{Empty label body.

Second paragraph.
\preformatted{x()}
\describe{\item{child}{Child body.}}}
\item{}{Plain empty label body.}
\item{last}{Last body.}
}}";

#[test]
fn empty_labels_remain_in_list_items() {
    let output = convert(EMPTY_LABELS, DescribeFormat::List);
    insta::assert_snapshot!(structure(&output, Options::empty()), @r#"
    + Heading { level: H1, id: None, classes: [], attrs: [] }
      Text("Labels")
    - Heading(H1)
    + Heading { level: H2, id: None, classes: [], attrs: [] }
      Text("Description")
    - Heading(H2)
    + List(None)
      + Item
        + Paragraph
          + Strong
            Text("first")
          - Strong
        - Paragraph
        + Paragraph
          Text("First body.")
        - Paragraph
      - Item
      + Item
        + Paragraph
          Text("\u{200b}")
        - Paragraph
        + Paragraph
          Text("Empty label body.")
        - Paragraph
        + Paragraph
          Text("Second paragraph.")
        - Paragraph
        + CodeBlock("")
          Text("x()\n")
        - CodeBlock
        + List(None)
          + Item
            + Paragraph
              + Strong
                Text("child")
              - Strong
            - Paragraph
            + Paragraph
              Text("Child body.")
            - Paragraph
          - Item
        - List(false)
      - Item
      + Item
        + Paragraph
          Text("\u{200b}")
        - Paragraph
        + Paragraph
          Text("Plain empty label body.")
        - Paragraph
      - Item
      + Item
        + Paragraph
          + Strong
            Text("last")
          - Strong
        - Paragraph
        + Paragraph
          Text("Last body.")
        - Paragraph
      - Item
    - List(false)
    "#);
}

#[test]
fn empty_labels_remain_in_definition_descriptions() {
    let output = convert(EMPTY_LABELS, DescribeFormat::DefinitionList);
    insta::assert_snapshot!(structure(&output, Options::ENABLE_DEFINITION_LIST), @r#"
    + Heading { level: H1, id: None, classes: [], attrs: [] }
      Text("Labels")
    - Heading(H1)
    + Heading { level: H2, id: None, classes: [], attrs: [] }
      Text("Description")
    - Heading(H2)
    + DefinitionList
      + DefinitionListTitle
        Text("first")
      - DefinitionListTitle
      + DefinitionListDefinition
        + Paragraph
          Text("First body.")
        - Paragraph
      - DefinitionListDefinition
      + DefinitionListTitle
        Text("\u{200b}")
      - DefinitionListTitle
      + DefinitionListDefinition
        + Paragraph
          Text("Empty label body.")
        - Paragraph
        + Paragraph
          Text("Second paragraph.")
        - Paragraph
        + CodeBlock("")
          Text("x()\n")
        - CodeBlock
        + DefinitionList
          + DefinitionListTitle
            Text("child")
          - DefinitionListTitle
          + DefinitionListDefinition
            Text("Child body.")
          - DefinitionListDefinition
        - DefinitionList
      - DefinitionListDefinition
      + DefinitionListTitle
        Text("\u{200b}")
      - DefinitionListTitle
      + DefinitionListDefinition
        + Paragraph
          Text("Plain empty label body.")
        - Paragraph
      - DefinitionListDefinition
      + DefinitionListTitle
        Text("last")
      - DefinitionListTitle
      + DefinitionListDefinition
        + Paragraph
          Text("Last body.")
        - Paragraph
      - DefinitionListDefinition
    - DefinitionList
    "#);
}

#[test]
fn empty_labels_keep_explicit_heading_boundaries() {
    let output = convert(EMPTY_LABELS, DescribeFormat::Headings);
    insta::assert_snapshot!(structure(&output, Options::empty()), @r#"
    + Heading { level: H1, id: None, classes: [], attrs: [] }
      Text("Labels")
    - Heading(H1)
    + Heading { level: H2, id: None, classes: [], attrs: [] }
      Text("Description")
    - Heading(H2)
    + Heading { level: H3, id: None, classes: [], attrs: [] }
      Text("first")
    - Heading(H3)
    + Paragraph
      Text("First body.")
    - Paragraph
    + Heading { level: H3, id: None, classes: [], attrs: [] }
    - Heading(H3)
    + Paragraph
      Text("Empty label body.")
    - Paragraph
    + Paragraph
      Text("Second paragraph.")
    - Paragraph
    + CodeBlock("")
      Text("x()\n")
    - CodeBlock
    + Heading { level: H4, id: None, classes: [], attrs: [] }
      Text("child")
    - Heading(H4)
    + Paragraph
      Text("Child body.")
    - Paragraph
    + Heading { level: H3, id: None, classes: [], attrs: [] }
    - Heading(H3)
    + Paragraph
      Text("Plain empty label body.")
    - Paragraph
    + Heading { level: H3, id: None, classes: [], attrs: [] }
      Text("last")
    - Heading(H3)
    + Paragraph
      Text("Last body.")
    - Paragraph
    "#);
}

#[test]
fn empty_argument_labels_stay_in_their_containers() {
    let parsed = rd2qmd_source::parse(
        r"\name{args}\title{Args}
\arguments{\item{x}{\describe{\item{\emph{ }}{Body.}}}}",
    )
    .unwrap();
    let options = RdConvertOptions {
        describe_format: DescribeFormat::List,
        arguments_format: ArgumentsFormat::List,
        ..Default::default()
    };
    let list = rd2qmd_core::convert_rd_document(parsed.document(), &options);
    let pipe = rd2qmd_core::convert_rd_document(
        parsed.document(),
        &RdConvertOptions {
            arguments_format: ArgumentsFormat::PipeTable,
            ..options
        },
    );
    insta::assert_snapshot!(structure(&list, Options::empty()), @r#"
    + Heading { level: H1, id: None, classes: [], attrs: [] }
      Text("Args")
    - Heading(H1)
    + Heading { level: H2, id: None, classes: [], attrs: [] }
      Text("Arguments")
    - Heading(H2)
    + List(None)
      + Item
        + Paragraph
          + Strong
            Code("x")
          - Strong
        - Paragraph
        + List(None)
          + Item
            + Paragraph
              Text("\u{200b}")
            - Paragraph
            + Paragraph
              Text("Body.")
            - Paragraph
          - Item
        - List(false)
      - Item
    - List(false)
    "#);
    insta::assert_snapshot!(structure(&pipe, Options::ENABLE_TABLES), @r#"
    + Heading { level: H1, id: None, classes: [], attrs: [] }
      Text("Args")
    - Heading(H1)
    + Heading { level: H2, id: None, classes: [], attrs: [] }
      Text("Arguments")
    - Heading(H2)
    + Table([Left, Left])
      + TableHead
        + TableCell
          Text("Argument")
        - TableCell
        + TableCell
          Text("Description")
        - TableCell
      - TableHead
      + TableRow
        + TableCell
          Code("x")
        - TableCell
        + TableCell
          Text("- ")
          Text("\u{200b}")
          Text(" ")
          InlineHtml(Borrowed("<br>"))
          Text("Body.")
        - TableCell
      - TableRow
    - Table
    "#);
}
