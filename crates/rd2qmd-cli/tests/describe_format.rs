//! Snapshot each describe format and parse compatible output without definition-list support.
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use rd2qmd_core::{ArgumentsFormat, DescribeFormat, RdConvertOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const MINIMAL: &str = r"\name{minimal}
\alias{minimal}
\title{Minimal}
\description{
\describe{
\item{\code{method}}{First paragraph.

Second paragraph with \code{x} and \strong{bold}.}
}
}";

fn convert_core(rd: &str) -> String {
    convert_core_as(rd, DescribeFormat::List)
}

fn convert_core_as(rd: &str, describe_format: DescribeFormat) -> String {
    let parsed = rd2qmd_source::parse(rd).unwrap();
    rd2qmd_core::convert_rd_document(
        parsed.document(),
        &RdConvertOptions {
            describe_format,
            arguments_format: ArgumentsFormat::List,
            ..Default::default()
        },
    )
}

#[test]
fn describe_minimal_prose_remains_prose() {
    for format in [DescribeFormat::List, DescribeFormat::Headings] {
        let output = convert_core_as(MINIMAL, format);
        for options in [
            Options::empty(),
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
        ] {
            let events: Vec<_> = Parser::new_ext(&output, options).collect();
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, Event::Start(Tag::CodeBlock(_))))
            );
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::Code(s) if s.as_ref() == "x"))
            );
            assert!(events.windows(3).any(|w| matches!(w,
            [Event::Start(Tag::Strong), Event::Text(s), Event::End(TagEnd::Strong)] if s.as_ref() == "bold"
        )));
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::Text(s) if s.contains("Second paragraph")))
            );
        }
    }
}

#[test]
fn describe_nested_blocks_preserve_code_and_item_membership() {
    let output = convert_core(include_str!("fixtures/describe_code_blocks.Rd"));
    let mut depth = 0;
    let mut code = None;
    let mut blocks = Vec::new();
    let mut nested_argument = false;
    let mut tail_in_method = false;
    for event in Parser::new(&output) {
        match event {
            Event::Start(Tag::Item) => depth += 1,
            Event::End(TagEnd::Item) => depth -= 1,
            Event::Start(Tag::CodeBlock(kind)) => {
                assert!(
                    matches!(kind, CodeBlockKind::Fenced(_)),
                    "unexpected indented code: {output}"
                );
                code = Some((depth, String::new()));
            }
            Event::Text(text) => {
                if let Some((_, body)) = &mut code {
                    body.push_str(&text);
                } else if text.contains("The first value.") {
                    assert_eq!(depth, 2);
                    nested_argument = true;
                } else if text.contains("That covers the") {
                    assert_eq!(depth, 1);
                    tail_in_method = true;
                }
            }
            Event::End(TagEnd::CodeBlock) => blocks.push(code.take().unwrap()),
            _ => {}
        }
    }
    assert!(nested_argument && tail_in_method);
    assert_eq!(
        blocks,
        vec![
            (
                1,
                "compute(a, b)\ncompute(a, b, extra = TRUE)\ncombine(a, b)\n".into()
            ),
            (2, "compute(1, 2)\n#> [1] 3\n".into()),
            (2, "compute(1, 2)\n#> [1] 3\n".into()),
        ]
    );
}

#[test]
fn describe_labels_preserve_links_and_inline_semantics() {
    let output = convert_core(
        r"\name{labels}\title{Labels}\description{
\describe{
\item{\href{https://example.com}{Read more}}{A description.}
\item{\emph{ordinary term}}{Another description.}
\item{\code{empty()}}{}
}}",
    );
    let events: Vec<_> = Parser::new(&output).collect();
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Link { dest_url, .. }) if dest_url.as_ref() == "https://example.com")));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::Emphasis)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Code(s) if s.as_ref() == "empty()"))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Code(s) if s.contains("ordinary term")))
    );
}

#[test]
fn describe_empty_and_padded_terms_do_not_create_markdown_syntax() {
    let output = convert_core(
        r"\name{terms}\title{Terms}\description{
\describe{
\item{}{Blank label body.}
\item{ spaced }{Whitespace label body.}
\item{\strong{bold}}{Strong label body.}
}}",
    );
    let events: Vec<_> = Parser::new(&output).collect();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Rule | Event::Start(Tag::CodeBlock(_))))
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Start(Tag::Item)))
            .count(),
        3
    );
    for term in ["spaced", "bold"] {
        assert!(events.windows(3).any(|w| matches!(w,
            [Event::Start(Tag::Strong), Event::Text(s), Event::End(TagEnd::Strong)] if s.as_ref() == term
        )));
    }
}

#[test]
fn describe_in_arguments_and_subsections_uses_selected_format() {
    let output = convert_core(
        r"\name{nested}\title{Nested}
\arguments{\item{x}{Argument prose.
\describe{\item{inner}{First paragraph.

Second paragraph.}}}}
\section{Details}{\subsection{Options}{\describe{\item{choice}{Choice prose.}}}}",
    );
    let mut depth = 0;
    let mut argument_body = false;
    let mut subsection_body = false;
    for event in Parser::new(&output) {
        match event {
            Event::Start(Tag::Item) => depth += 1,
            Event::End(TagEnd::Item) => depth -= 1,
            Event::Start(Tag::CodeBlock(_)) => panic!("Prose became code: {output}"),
            Event::Text(s) if s.contains("Second paragraph.") => {
                assert_eq!(depth, 2);
                argument_body = true;
            }
            Event::Text(s) if s.contains("Choice prose.") => {
                assert_eq!(depth, 1);
                subsection_body = true;
            }
            _ => {}
        }
    }
    assert!(argument_body && subsection_body);
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rd2qmd_describe_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_cli(cwd: &Path, args: &[&str]) {
    let result = Command::new(env!("CARGO_BIN_EXE_rd2qmd"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// Render the same ggproto-style fixture through the CLI in every describe format.
/// Keep both Markdown and Quarto output visible for review, including code fences,
/// nested arguments, and ordered/unordered examples inside a method description.
#[test]
fn describe_format_snapshots() {
    let scratch = Scratch::new();
    fs::write(
        scratch.0.join("describe.Rd"),
        include_str!("fixtures/describe_code_blocks.Rd"),
    )
    .unwrap();
    for format in ["definition-list", "list", "headings"] {
        for extension in ["md", "qmd"] {
            let filename = format!("output.{extension}");
            run_cli(
                &scratch.0,
                &[
                    "convert",
                    "describe.Rd",
                    "--no-config",
                    "--no-frontmatter",
                    "--describe-format",
                    format,
                    "--format",
                    extension,
                    "-o",
                    &filename,
                ],
            );
            let output = fs::read_to_string(scratch.0.join(filename)).unwrap();
            insta::assert_snapshot!(
                format!("describe_{}_{extension}", format.replace('-', "_")),
                output
            );
        }
    }
}

#[test]
fn describe_headings_in_default_argument_list_table() {
    let scratch = Scratch::new();
    fs::write(
        scratch.0.join("arguments.Rd"),
        r"\name{options}\title{Options}
\arguments{
\item{x}{\describe{\item{\code{first}}{First body.
\describe{\item{nested}{Nested body.}}
\preformatted{first(x)}
}\item{second}{Second body.}}}
\item{y}{Introductory prose.
\describe{\item{third}{Third body.}}}
}",
    )
    .unwrap();
    run_cli(
        &scratch.0,
        &[
            "convert",
            "arguments.Rd",
            "--no-config",
            "--no-frontmatter",
            "--describe-format",
            "headings",
            "-o",
            "arguments.qmd",
        ],
    );
    let output = fs::read_to_string(scratch.0.join("arguments.qmd")).unwrap();
    assert!(output.contains("{.list-table header-rows=1}"));
    for heading in ["### `first`", "#### nested", "### second", "### third"] {
        assert!(output.contains(heading), "Missing {heading}: {output}");
    }
    for body in [
        "First body.",
        "Nested body.",
        "Second body.",
        "Third body.",
        "first(x)",
    ] {
        assert!(output.contains(body), "Missing {body}: {output}");
    }
    insta::assert_snapshot!("describe_headings_default_argument_list_table", output);
}

fn assert_describe_output(path: &Path, format: &str) {
    let output = fs::read_to_string(path).unwrap();
    let term = if format == "headings" {
        "### `method`"
    } else {
        "- **`method`**"
    };
    assert!(output.contains(term), "{output}");
    assert!(
        !Parser::new(&output).any(|e| matches!(e, Event::Start(Tag::CodeBlock(_)))),
        "{output}"
    );
}

#[test]
fn describe_cli_config_precedence_and_rd_ast_directory_routes() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    fs::create_dir(root.join("rd")).unwrap();
    fs::write(root.join("rd/minimal.Rd"), MINIMAL).unwrap();
    for format in ["list", "headings"] {
        fs::write(
            root.join("_rd2qmd.toml"),
            format!("[output]\ndescribe_format = '{format}'\nfrontmatter = false\n"),
        )
        .unwrap();
        run_cli(root, &["convert", "rd/minimal.Rd", "-o", "single.md"]);
        assert_describe_output(&root.join("single.md"), format);
        run_cli(
            root,
            &["convert", "rd", "--no-external-links", "-o", "rd-output"],
        );
        assert_describe_output(&root.join("rd-output/minimal.qmd"), format);
        run_cli(root, &["parse", "rd", "-o", "ast"]);
        run_cli(root, &["convert", "ast/minimal.json", "-o", "ast.md"]);
        assert_describe_output(&root.join("ast.md"), format);
        run_cli(
            root,
            &[
                "convert",
                "ast",
                "--input-format",
                "ast",
                "--no-external-links",
                "-o",
                "ast-output",
            ],
        );
        assert_describe_output(&root.join("ast-output/minimal.qmd"), format);

        // An explicit default value must override the config's non-default value.
        run_cli(
            root,
            &[
                "convert",
                "rd/minimal.Rd",
                "--describe-format",
                "definition-list",
                "-o",
                "definition.md",
            ],
        );
        let definition = fs::read_to_string(root.join("definition.md")).unwrap();
        assert!(definition.contains("`method`\n:   First paragraph."));
        run_cli(
            root,
            &[
                "convert",
                "rd/minimal.Rd",
                "--no-config",
                "--no-frontmatter",
                "-o",
                "default.md",
            ],
        );
        assert_eq!(
            definition,
            fs::read_to_string(root.join("default.md")).unwrap()
        );

        fs::write(
            root.join("_rd2qmd.toml"),
            "[output]\ndescribe_format = 'definition-list'\n",
        )
        .unwrap();
        run_cli(
            root,
            &[
                "convert",
                "rd/minimal.Rd",
                "--describe-format",
                format,
                "--no-frontmatter",
                "-o",
                "override.md",
            ],
        );
        assert_describe_output(&root.join("override.md"), format);
    }
}

fn parsed_headings(output: &str) -> Vec<(u8, String)> {
    let mut headings = Vec::new();
    let mut current = None;
    for event in Parser::new(output) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((level as u8, String::new()))
            }
            Event::Text(s) | Event::Code(s) => {
                if let Some((_, title)) = &mut current {
                    title.push_str(&s);
                }
            }
            Event::End(TagEnd::Heading(_)) => headings.push(current.take().unwrap()),
            _ => {}
        }
    }
    headings
}

#[test]
fn describe_headings_follow_section_and_description_depth() {
    let output = convert_core_as(
        r"\name{headings}\title{Headings}
\details{\describe{\item{detail}{Detail body.}}}
\section{Methods}{
\describe{\item{\code{method()}}{Method body.
\describe{\item{\href{https://example.com}{argument}}{Argument body.}}
\preformatted{method(x)
}
}\item{second}{Second body.}}
\subsection{Advanced}{\describe{\item{option}{Option body.}}}
}",
        DescribeFormat::Headings,
    );
    insta::assert_snapshot!("describe_headings_section_depth", output);
    assert_eq!(
        parsed_headings(&output),
        vec![
            (1, "Headings".into()),
            (2, "Details".into()),
            (3, "detail".into()),
            (2, "Methods".into()),
            (3, "method()".into()),
            (4, "argument".into()),
            (3, "second".into()),
            (3, "Advanced".into()),
            (4, "option".into()),
        ]
    );
    let events: Vec<_> = Parser::new(&output).collect();
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Link { dest_url, .. }) if dest_url.as_ref() == "https://example.com")));
    assert!(events.windows(3).any(|w| matches!(w,
        [Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_))), Event::Text(s), Event::End(TagEnd::CodeBlock)] if s.as_ref() == "method(x)\n"
    )));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::List(_))))
    );
}

#[test]
fn describe_headings_below_h6_preserve_nesting_as_lists() {
    let mut body = "Deep prose.\n\n\\preformatted{deep(x)}".to_string();
    for i in (0..7).rev() {
        body = format!("\\describe{{\\item{{level{i}}}{{{body}}}}}");
    }
    let rd = format!("\\name{{deep}}\\title{{Deep}}\\section{{Methods}}{{{body}}}");
    let output = convert_core_as(&rd, DescribeFormat::Headings);
    insta::assert_snapshot!("describe_headings_beyond_h6", output);
    assert_eq!(
        parsed_headings(&output),
        vec![
            (1, "Deep".into()),
            (2, "Methods".into()),
            (3, "level0".into()),
            (4, "level1".into()),
            (5, "level2".into()),
            (6, "level3".into()),
        ]
    );
    let mut depth = 0;
    let mut deep_prose = false;
    let mut deep_code = false;
    for event in Parser::new(&output) {
        match event {
            Event::Start(Tag::Item) => depth += 1,
            Event::End(TagEnd::Item) => depth -= 1,
            Event::Text(s) if s.contains("Deep prose.") => {
                assert_eq!(depth, 3);
                deep_prose = true;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                assert!(matches!(kind, CodeBlockKind::Fenced(_)));
                assert_eq!(depth, 3);
                deep_code = true;
            }
            _ => {}
        }
    }
    assert!(deep_prose && deep_code);
}

#[test]
fn describe_arguments_format_combinations() {
    let parsed = rd2qmd_source::parse(
        r"\name{formats}\title{Formats}
\description{\describe{\item{outside}{Outside body.}}}
\arguments{\item{x}{\describe{
\item{\href{https://example.com}{term}}{First paragraph.

Second paragraph.
\describe{\item{nested}{Nested body.}}
\preformatted{x | y}
}}}}",
    )
    .unwrap();
    for arguments_format in [
        ArgumentsFormat::PipeTable,
        ArgumentsFormat::GridTable,
        ArgumentsFormat::ListTable,
        ArgumentsFormat::List,
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
            for text in [
                "Outside body.",
                "First paragraph.",
                "Second paragraph.",
                "Nested body.",
                "nested",
                "https://example.com",
            ] {
                assert!(
                    output.contains(text),
                    "{arguments_format:?}/{describe_format:?}: {output}"
                );
            }
            if describe_format == DescribeFormat::Headings {
                assert!(output.contains("### outside"));
                if arguments_format != ArgumentsFormat::PipeTable {
                    assert!(output.contains("### [term](https://example.com)"));
                    assert!(output.contains("#### nested"));
                    assert!(output.contains("x | y"));
                }
            }
            if arguments_format == ArgumentsFormat::PipeTable {
                let mut in_cell = false;
                let mut strong = 0;
                let mut link = false;
                let mut code = false;
                for event in Parser::new_ext(&output, Options::ENABLE_TABLES) {
                    match event {
                        Event::Start(Tag::TableCell) => in_cell = true,
                        Event::End(TagEnd::TableCell) => in_cell = false,
                        Event::Start(Tag::Strong) if in_cell => strong += 1,
                        Event::Start(Tag::Link { dest_url, .. }) if in_cell => {
                            link |= dest_url.as_ref() == "https://example.com";
                        }
                        Event::Code(value) if in_cell => code |= value.as_ref() == "x | y",
                        Event::Text(value) if in_cell => assert!(!value.contains("###")),
                        _ => {}
                    }
                }
                assert!(link && code, "{output}");
                if describe_format != DescribeFormat::DefinitionList {
                    assert_eq!(strong, 2, "{output}");
                }
                if describe_format == DescribeFormat::Headings {
                    insta::assert_snapshot!("describe_headings_argument_pipe_table", output);
                }
            }
        }
    }
}
