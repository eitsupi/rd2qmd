//! Boundary whitespace must preserve both words and inline semantics.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
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
\describe{\item{\emph{ term }}{Argument body.}}
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
            for body in ["Label body.", "Empty label body.", "Argument body."] {
                assert!(output.contains(body), "{output}");
            }
            for invalid in [
                "_ foo _",
                "* foo *",
                "** bar **",
                "_ label _",
                "* term *",
                "****",
            ] {
                assert!(!output.contains(invalid), "{output}");
            }
            // Parse only syntax supported by CommonMark/GFM. The other formats
            // are checked in snapshots, and manually with their Pandoc readers.
            if describe_format != DescribeFormat::DefinitionList
                && matches!(
                    arguments_format,
                    ArgumentsFormat::List | ArgumentsFormat::PipeTable
                )
            {
                for options in [
                    Options::ENABLE_TABLES,
                    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
                ] {
                    let mut emphasized = String::new();
                    let mut strong_text = String::new();
                    let mut strong_depth = 0;
                    let mut visible = String::new();
                    let mut depth = 0;
                    let mut code = false;
                    let mut link = false;
                    for event in Parser::new_ext(&output, options) {
                        match event {
                            Event::Start(Tag::Strong) => strong_depth += 1,
                            Event::End(TagEnd::Strong) => strong_depth -= 1,
                            Event::Start(Tag::Emphasis) => depth += 1,
                            Event::End(TagEnd::Emphasis) => depth -= 1,
                            Event::Start(Tag::Link { dest_url, .. }) => {
                                link |= dest_url.as_ref() == "https://example.com"
                            }
                            Event::Text(text) => {
                                visible.push_str(&text);
                                if strong_depth > 0 {
                                    strong_text.push_str(&text);
                                }
                                if depth > 0 {
                                    emphasized.push_str(&text);
                                }
                            }
                            Event::Code(text) => code |= depth > 0 && text.as_ref() == "x | y",
                            _ => {}
                        }
                    }
                    for word in ["foo", "nested", "linked", "label", "term"] {
                        assert!(emphasized.contains(word), "{output}");
                    }
                    for phrase in ["A foo B.", "A bar B.", "A nested B.", "A linked B.", "A B."] {
                        assert!(visible.contains(phrase), "{output}");
                    }
                    assert!(strong_text.contains("bar") && strong_text.contains("nested"));
                    assert!(code && link, "{output}");
                }
            }
            if arguments_format == ArgumentsFormat::List || describe_format == DescribeFormat::List
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
