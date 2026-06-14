use super::*;
use crate::ast::{FigureOptions, SpecialChar};

#[test]
fn test_empty_document() {
    let doc = parse("").unwrap();
    assert!(doc.sections.is_empty());
}

#[test]
fn test_simple_section() {
    let doc = parse("\\name{test}").unwrap();
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].tag, SectionTag::Name);
}

#[test]
fn test_multiple_sections() {
    let doc = parse("\\name{foo}\n\\title{Bar}").unwrap();
    assert_eq!(doc.sections.len(), 2);
    assert_eq!(doc.sections[0].tag, SectionTag::Name);
    assert_eq!(doc.sections[1].tag, SectionTag::Title);
}

#[test]
fn test_inline_code() {
    let doc = parse("\\description{Use \\code{foo} here}").unwrap();
    assert_eq!(doc.sections.len(), 1);
    let content = &doc.sections[0].content;
    assert!(content.len() >= 3); // Text, Code, Text
}

#[test]
fn test_href() {
    let doc = parse("\\description{\\href{https://example.com}{Example}}").unwrap();
    let content = &doc.sections[0].content;
    assert!(matches!(content[0], RdNode::Href { .. }));
}

#[test]
fn test_itemize() {
    let doc = parse("\\details{\\itemize{\\item One\\item Two}}").unwrap();
    let content = &doc.sections[0].content;
    assert!(matches!(&content[0], RdNode::Itemize(_)));
}

#[test]
fn test_subsection() {
    let doc = parse("\\details{\\subsection{Sub}{Content here}}").unwrap();
    let content = &doc.sections[0].content;
    assert!(matches!(&content[0], RdNode::Subsection { .. }));
}

#[test]
fn test_special_chars() {
    let doc = parse("\\description{\\R and \\dots}").unwrap();
    let content = &doc.sections[0].content;
    assert!(
        content
            .iter()
            .any(|n| matches!(n, RdNode::Special(SpecialChar::R)))
    );
    assert!(
        content
            .iter()
            .any(|n| matches!(n, RdNode::Special(SpecialChar::Dots)))
    );
}

#[test]
fn test_real_rd_file() {
    let source = r#"
\name{test}
\alias{test}
\title{Test Function}
\description{
This is a test with \code{inline code} and a \href{https://example.com}{link}.
}
\usage{
test(x, y = TRUE)
}
\arguments{
\item{x}{The first argument}
\item{y}{The second argument}
}
"#;
    let doc = parse(source).unwrap();
    assert!(doc.sections.len() >= 5);
}

#[test]
fn test_dontshow_with_escaped_braces() {
    // Test that \{ inside \dontshow becomes Text("{")
    let doc = parse(r#"\examples{\dontshow{if (FALSE) \{ # test}}"#).unwrap();
    let content = &doc.sections[0].content;
    assert_eq!(content.len(), 1, "Expected exactly one node");
    if let RdNode::DontShow(children) = &content[0] {
        // The content should include the escaped brace as text
        let has_text_with_brace = children.iter().any(|n| {
            if let RdNode::Text(s) = n {
                s.contains('{')
            } else {
                false
            }
        });
        assert!(
            has_text_with_brace,
            "Expected Text node containing '{{' from \\{{"
        );
    } else {
        panic!("Expected DontShow node, got {:?}", content[0]);
    }
}

#[test]
fn test_dontshow_end_wrapper() {
    // Test that \} inside \dontshow becomes Text("}")
    let doc = parse(r#"\examples{\dontshow{\}) # test}}"#).unwrap();
    let content = &doc.sections[0].content;
    assert_eq!(content.len(), 1, "Expected exactly one node");
    if let RdNode::DontShow(children) = &content[0] {
        // The first child should be Text starting with }
        if let Some(RdNode::Text(s)) = children.first() {
            assert!(
                s.starts_with('}'),
                "Expected text starting with '}}', got '{}'",
                s
            );
        } else {
            panic!("Expected first child to be Text, got {:?}", children);
        }
    } else {
        panic!("Expected DontShow node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for \figure tag parsing
// ========================================================================

#[test]
fn test_figure_simple_no_options() {
    // Form 1: \figure{filename} - no second argument
    let doc = parse(r#"\description{\figure{Rlogo.svg}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Figure { file, options } = &content[0] {
        assert_eq!(file, "Rlogo.svg");
        assert!(options.is_none());
    } else {
        panic!("Expected Figure node, got {:?}", content[0]);
    }
}

#[test]
fn test_figure_simple_with_alt_text() {
    // Form 2: \figure{filename}{alternate text}
    let doc = parse(r#"\description{\figure{Rlogo.svg}{R logo}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Figure { file, options } = &content[0] {
        assert_eq!(file, "Rlogo.svg");
        assert_eq!(options, &Some(FigureOptions::AltText("R logo".to_string())));
    } else {
        panic!("Expected Figure node, got {:?}", content[0]);
    }
}

#[test]
fn test_figure_expert_form() {
    // Form 3: \figure{filename}{options: string}
    // Note: "options:" prefix is stripped, remaining string is stored
    let doc =
        parse(r#"\description{\figure{Rlogo.svg}{options: width=100 alt="R logo"}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Figure { file, options } = &content[0] {
        assert_eq!(file, "Rlogo.svg");
        assert_eq!(
            options,
            &Some(FigureOptions::ExpertOptions(
                r#"width=100 alt="R logo""#.to_string()
            ))
        );
    } else {
        panic!("Expected Figure node, got {:?}", content[0]);
    }
}

#[test]
fn test_figure_lifecycle_badge_style() {
    // Lifecycle badge format with single quotes
    // Note: "options:" prefix is stripped
    let doc =
        parse(r#"\description{\figure{lifecycle-deprecated.svg}{options: alt='[Deprecated]'}}"#)
            .unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Figure { file, options } = &content[0] {
        assert_eq!(file, "lifecycle-deprecated.svg");
        assert_eq!(
            options,
            &Some(FigureOptions::ExpertOptions(
                "alt='[Deprecated]'".to_string()
            ))
        );
    } else {
        panic!("Expected Figure node, got {:?}", content[0]);
    }
}

#[test]
fn test_figure_with_bracket_arg_fallback() {
    // Bracket syntax fallback: \figure[alt]{filename}
    let doc = parse(r#"\description{\figure[R logo]{Rlogo.svg}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Figure { file, options } = &content[0] {
        assert_eq!(file, "Rlogo.svg");
        // Bracket arg becomes options when no brace arg is present (treated as simple form)
        assert_eq!(options, &Some(FigureOptions::AltText("R logo".to_string())));
    } else {
        panic!("Expected Figure node, got {:?}", content[0]);
    }
}

#[test]
fn test_figure_options_starting_with_options_word() {
    // Edge case: text starting with "options" but not "options:" should be simple form
    let doc = parse(r#"\description{\figure{file.png}{options are shown here}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Figure { file, options } = &content[0] {
        assert_eq!(file, "file.png");
        assert_eq!(
            options,
            &Some(FigureOptions::AltText("options are shown here".to_string()))
        );
    } else {
        panic!("Expected Figure node, got {:?}", content[0]);
    }
}

#[test]
fn test_figure_options_colon_no_space() {
    // Edge case: "options:" without space should be simple form (per spec: must be followed by space)
    let doc = parse(r#"\description{\figure{file.png}{options:nospace}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Figure { file, options } = &content[0] {
        assert_eq!(file, "file.png");
        assert_eq!(
            options,
            &Some(FigureOptions::AltText("options:nospace".to_string()))
        );
    } else {
        panic!("Expected Figure node, got {:?}", content[0]);
    }
}

// Link parsing tests

#[test]
fn test_link_simple() {
    // Form 1: \link{topic}
    let doc = parse(r#"\description{\link{foo}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Link {
        package,
        topic,
        text,
    } = &content[0]
    {
        assert_eq!(package, &None);
        assert_eq!(topic, "foo");
        assert_eq!(text, &None);
    } else {
        panic!("Expected Link node, got {:?}", content[0]);
    }
}

#[test]
fn test_link_with_package() {
    // Form 2: \link[pkg]{topic}
    let doc = parse(r#"\description{\link[dplyr]{filter}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Link {
        package,
        topic,
        text,
    } = &content[0]
    {
        assert_eq!(package, &Some("dplyr".to_string()));
        assert_eq!(topic, "filter");
        assert_eq!(text, &None);
    } else {
        panic!("Expected Link node, got {:?}", content[0]);
    }
}

#[test]
fn test_link_with_package_and_topic() {
    // Form 3: \link[pkg:bar]{text} - topic comes from pkg:bar, brace content is display text
    let doc = parse(r#"\description{\link[rlang:abort]{abort function}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Link {
        package,
        topic,
        text,
    } = &content[0]
    {
        assert_eq!(package, &Some("rlang".to_string()));
        assert_eq!(topic, "abort");
        assert!(text.is_some());
        // Display text should be "abort function"
        if let Some(text_nodes) = text {
            if let RdNode::Text(s) = &text_nodes[0] {
                assert_eq!(s, "abort function");
            } else {
                panic!("Expected Text node in display text");
            }
        }
    } else {
        panic!("Expected Link node, got {:?}", content[0]);
    }
}

#[test]
fn test_link_with_equals_dest() {
    // Form 4: \link[=dest]{text} - link to dest, display text
    let doc = parse(r#"\description{\link[=as_polars_series]{as_polars_series()}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Link {
        package,
        topic,
        text,
    } = &content[0]
    {
        assert_eq!(package, &None);
        assert_eq!(topic, "as_polars_series");
        assert!(text.is_some());
        if let Some(text_nodes) = text {
            if let RdNode::Text(s) = &text_nodes[0] {
                assert_eq!(s, "as_polars_series()");
            } else {
                panic!("Expected Text node in display text");
            }
        }
    } else {
        panic!("Expected Link node, got {:?}", content[0]);
    }
}

#[test]
fn test_link_pkg_topic_with_hyphen() {
    // Real-world case: \link[rlang:dyn-dots]{dynamic dots}
    let doc = parse(r#"\description{\link[rlang:dyn-dots]{dynamic dots}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Link {
        package,
        topic,
        text,
    } = &content[0]
    {
        assert_eq!(package, &Some("rlang".to_string()));
        assert_eq!(topic, "dyn-dots");
        assert!(text.is_some());
        if let Some(text_nodes) = text {
            if let RdNode::Text(s) = &text_nodes[0] {
                assert_eq!(s, "dynamic dots");
            } else {
                panic!("Expected Text node in display text");
            }
        }
    } else {
        panic!("Expected Link node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for special characters
// ========================================================================

#[test]
fn test_ldots() {
    // \ldots is an alias for \dots and should produce SpecialChar::Dots
    let doc = parse(r#"\description{a, b, \ldots{}, z}"#).unwrap();
    let content = &doc.sections[0].content;
    assert!(
        content
            .iter()
            .any(|n| matches!(n, RdNode::Special(SpecialChar::Dots))),
        "Expected Dots special char, got: {:?}",
        content
    );
}

// ========================================================================
// Tests for preformatted text
// ========================================================================

#[test]
fn test_preformatted() {
    let doc = parse(r#"\details{\preformatted{x <- 1}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Preformatted(s) = &content[0] {
        assert_eq!(s, "x <- 1");
    } else {
        panic!("Expected Preformatted node, got {:?}", content[0]);
    }
}

#[test]
fn test_preformatted_preserves_whitespace() {
    let doc = parse(
        r#"\details{\preformatted{
  line1
    line2
}}"#,
    )
    .unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Preformatted(s) = &content[0] {
        assert!(s.contains("  line1"));
        assert!(s.contains("    line2"));
    } else {
        panic!("Expected Preformatted node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for special section tags
// ========================================================================

#[test]
fn test_concept_section() {
    let doc = parse(r#"\concept{data analysis}"#).unwrap();
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].tag, SectionTag::Concept);
}

#[test]
fn test_format_section() {
    let doc = parse(r#"\format{A data frame with 10 rows.}"#).unwrap();
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].tag, SectionTag::Format);
}

#[test]
fn test_source_section() {
    let doc = parse(r#"\source{Data from example.com}"#).unwrap();
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].tag, SectionTag::Source);
}

#[test]
fn test_encoding_section() {
    let doc = parse(r#"\encoding{UTF-8}"#).unwrap();
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].tag, SectionTag::Encoding);
}

#[test]
fn test_doctype_section() {
    let doc = parse(r#"\docType{data}"#).unwrap();
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].tag, SectionTag::DocType);
}

#[test]
fn test_rdversion_section() {
    let doc = parse(r#"\RdVersion{1.1}"#).unwrap();
    assert_eq!(doc.sections.len(), 1);
    // Note: RdVersion is case-sensitive in parse, so it might be Unknown
    // If parser treats it as Unknown, that's expected
}

// ========================================================================
// Tests for testonly (alias for dontshow)
// ========================================================================

#[test]
fn test_testonly() {
    let doc = parse(r#"\examples{\testonly{stopifnot(TRUE)}}"#).unwrap();
    let content = &doc.sections[0].content;
    assert!(matches!(&content[0], RdNode::DontShow(_)));
}

// ========================================================================
// Tests for \enc (encoding-dependent text)
// ========================================================================

#[test]
fn test_enc_basic() {
    let doc = parse(r#"\description{\enc{Jöreskog}{Joreskog}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Enc { encoded, fallback } = &content[0] {
        assert_eq!(encoded, "Jöreskog");
        assert_eq!(fallback, "Joreskog");
    } else {
        panic!("Expected Enc node, got {:?}", content[0]);
    }
}

#[test]
fn test_enc_dash() {
    let doc = parse(r#"\description{\enc{–}{--}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Enc { encoded, fallback } = &content[0] {
        assert_eq!(encoded, "–"); // en-dash
        assert_eq!(fallback, "--");
    } else {
        panic!("Expected Enc node, got {:?}", content[0]);
    }
}

#[test]
fn test_enc_single_arg() {
    // If only one argument is provided, both should be the same
    let doc = parse(r#"\description{\enc{text}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Enc { encoded, fallback } = &content[0] {
        assert_eq!(encoded, "text");
        assert_eq!(fallback, "text");
    } else {
        panic!("Expected Enc node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for empty arguments
// ========================================================================

#[test]
fn test_empty_code() {
    let doc = parse(r#"\description{\code{}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Code(children) = &content[0] {
        assert!(children.is_empty());
    } else {
        panic!("Expected Code node, got {:?}", content[0]);
    }
}

#[test]
fn test_empty_emph() {
    let doc = parse(r#"\description{\emph{}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Emph(children) = &content[0] {
        assert!(children.is_empty());
    } else {
        panic!("Expected Emph node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for nested formatting
// ========================================================================

#[test]
fn test_nested_code_in_emph() {
    let doc = parse(r#"\description{\emph{use \code{foo}}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Emph(children) = &content[0] {
        assert!(children.iter().any(|n| matches!(n, RdNode::Code(_))));
    } else {
        panic!("Expected Emph node, got {:?}", content[0]);
    }
}

#[test]
fn test_nested_emph_in_strong() {
    let doc = parse(r#"\description{\strong{very \emph{important}}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Strong(children) = &content[0] {
        assert!(children.iter().any(|n| matches!(n, RdNode::Emph(_))));
    } else {
        panic!("Expected Strong node, got {:?}", content[0]);
    }
}

#[test]
fn test_link_in_code() {
    let doc = parse(r#"\description{\code{\link{foo}}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Code(children) = &content[0] {
        assert!(children.iter().any(|n| matches!(n, RdNode::Link { .. })));
    } else {
        panic!("Expected Code node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for multiple arguments (eqn, deqn)
// ========================================================================

#[test]
fn test_eqn_single_arg() {
    let doc = parse(r#"\description{\eqn{\alpha}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Eqn { latex, ascii } = &content[0] {
        assert_eq!(latex, r"\alpha");
        assert!(ascii.is_none());
    } else {
        panic!("Expected Eqn node, got {:?}", content[0]);
    }
}

#[test]
fn test_eqn_two_args() {
    let doc = parse(r#"\description{\eqn{x^2}{x squared}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Eqn { latex, ascii } = &content[0] {
        assert_eq!(latex, "x^2");
        assert_eq!(ascii, &Some("x squared".to_string()));
    } else {
        panic!("Expected Eqn node, got {:?}", content[0]);
    }
}

#[test]
fn test_deqn_single_arg() {
    let doc = parse(r#"\details{\deqn{\sum_{i=1}^n x_i}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Deqn { latex, ascii } = &content[0] {
        assert!(latex.contains(r"\sum"));
        assert!(ascii.is_none());
    } else {
        panic!("Expected Deqn node, got {:?}", content[0]);
    }
}

#[test]
fn test_deqn_two_args() {
    let doc = parse(r#"\details{\deqn{\sum x_i}{sum(x)}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Deqn { latex, ascii } = &content[0] {
        assert!(latex.contains(r"\sum"));
        assert_eq!(ascii, &Some("sum(x)".to_string()));
    } else {
        panic!("Expected Deqn node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for verb
// ========================================================================

#[test]
fn test_verb() {
    let doc = parse(r#"\description{\verb{x <- 1}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Verb(s) = &content[0] {
        assert_eq!(s, "x <- 1");
    } else {
        panic!("Expected Verb node, got {:?}", content[0]);
    }
}

#[test]
fn test_verb_preserves_special_chars() {
    let doc = parse(r#"\description{\verb{foo{bar}baz}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Verb(s) = &content[0] {
        assert_eq!(s, "foo{bar}baz");
    } else {
        panic!("Expected Verb node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for out
// ========================================================================

#[test]
fn test_out() {
    let doc = parse(r#"\description{\out{<b>bold</b>}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Out(s) = &content[0] {
        assert_eq!(s, "<b>bold</b>");
    } else {
        panic!("Expected Out node, got {:?}", content[0]);
    }
}

// ========================================================================
// Tests for Sexpr
// ========================================================================

#[test]
fn test_sexpr_no_options() {
    let doc = parse(r#"\description{\Sexpr{1 + 1}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Sexpr { options, code } = &content[0] {
        assert!(options.is_none());
        assert_eq!(code, "1 + 1");
    } else {
        panic!("Expected Sexpr node, got {:?}", content[0]);
    }
}

#[test]
fn test_sexpr_with_options() {
    let doc = parse(r#"\description{\Sexpr[results=rd]{paste("a", "b")}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Sexpr { options, code } = &content[0] {
        assert_eq!(options, &Some("results=rd".to_string()));
        assert!(code.contains("paste"));
    } else {
        panic!("Expected Sexpr node, got {:?}", content[0]);
    }
}

// ========================================================================
// Regression tests for GitHub issue #20: zero-arg macro terminator and
// literal braces in deparsed Rd examples
// ========================================================================

/// Rd macro names consist only of ASCII alphanumeric characters.
/// A zero-arg macro immediately followed by any non-alphanumeric character
/// must be recognized as the macro + preserved literal text, regardless of
/// which punctuation character follows.
#[test]
fn test_zero_arg_macro_terminates_at_non_alphanumeric() {
    // (input, expected terminator char)
    let cases: &[(&str, char)] = &[
        (r#"\usage{\dots)}"#, ')'),
        (r#"\usage{\dots,}"#, ','),
        (r#"\usage{\dots.}"#, '.'),
        (r#"\usage{\ldots)}"#, ')'),
    ];
    for &(input, expected_char) in cases {
        let doc = parse(input).unwrap();
        let content = &doc.sections[0].content;
        let has_dots = content
            .iter()
            .any(|n| matches!(n, RdNode::Special(SpecialChar::Dots)));
        assert!(
            has_dots,
            "Input {input:?}: expected SpecialChar::Dots, got: {content:?}"
        );
        let text: String = content
            .iter()
            .filter_map(|n| {
                if let RdNode::Text(s) = n {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(
            text.contains(expected_char),
            "Input {input:?}: expected {expected_char:?} preserved in text, got: {text:?}"
        );
    }
}

/// Literal `{` `}` in `\examples{}` must be preserved in the AST text
/// (they are R code syntax, not Rd grouping braces).
#[test]
fn test_examples_preserves_literal_braces() {
    let doc = parse(r#"\examples{hilbert <- function(n) { i <- 1:n; 1 / outer(i - 1, i, `+`) }}"#)
        .unwrap();
    let content = &doc.sections[0].content;
    let text = content
        .iter()
        .filter_map(|n| {
            if let RdNode::Text(s) = n {
                Some(s.as_str())
            } else {
                None
            }
        })
        .collect::<String>();
    assert!(
        text.contains("{ i <- 1:n"),
        "Expected literal '{{' preserved in examples, full text: {:?}",
        text
    );
    assert!(
        text.ends_with('}') || text.contains("} }") || text.contains("`) }"),
        "Expected literal '}}' preserved in examples, full text: {:?}",
        text
    );
}

/// Literal `{` `}` in `\usage{}` must also be preserved.
#[test]
fn test_usage_preserves_literal_braces() {
    let doc = parse(r#"\usage{f <- function(x) { x + 1 }}"#).unwrap();
    let content = &doc.sections[0].content;
    let text = content
        .iter()
        .filter_map(|n| {
            if let RdNode::Text(s) = n {
                Some(s.as_str())
            } else {
                None
            }
        })
        .collect::<String>();
    assert!(
        text.contains("{ x + 1 }"),
        "Expected literal braces preserved in usage, full text: {:?}",
        text
    );
}

/// Braces in `\dontrun{}` inside `\examples{}` should also be preserved.
#[test]
fn test_examples_dontrun_preserves_braces() {
    let doc = parse(r#"\examples{\dontrun{f <- function(x) { x + 1 }}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::DontRun(children) = &content[0] {
        let text = children
            .iter()
            .filter_map(|n| {
                if let RdNode::Text(s) = n {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect::<String>();
        assert!(
            text.contains("{ x + 1 }"),
            "Expected literal braces preserved inside \\dontrun, full text: {:?}",
            text
        );
    } else {
        panic!("Expected DontRun node, got {:?}", content[0]);
    }
}

/// `\dots{}` in a code section must produce `SpecialChar::Dots` only —
/// the empty `{}` terminator must not be preserved as literal text `"{}"`.
#[test]
fn test_zero_arg_macro_empty_brace_terminator_not_preserved() {
    for input in &[
        r#"\usage{f(\dots{})}"#,
        r#"\usage{f(\ldots{})}"#,
        r#"\examples{f(\dots{})}"#,
    ] {
        let doc = parse(input).unwrap();
        let content = &doc.sections[0].content;
        let has_dots = content
            .iter()
            .any(|n| matches!(n, RdNode::Special(SpecialChar::Dots)));
        assert!(has_dots, "Input {input:?}: expected SpecialChar::Dots");
        let text: String = content
            .iter()
            .filter_map(|n| {
                if let RdNode::Text(s) = n {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(
            !text.contains("{}"),
            "Input {input:?}: empty {{}} terminator must not appear in text, got: {text:?}"
        );
    }
}

/// `\tab{}` and `\cr{}` inside `\tabular{}` must consume the empty `{}`
/// without leaving stray tokens that corrupt the table parse.
#[test]
fn test_tabular_tab_cr_with_empty_brace_terminator() {
    // \tab{} — empty brace after cell separator
    let doc = parse(r#"\details{\tabular{ll}{a \tab{} b \cr{} c \tab{} d \cr}}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Tabular { alignment, rows } = &content[0] {
        assert_eq!(alignment, "ll");
        assert_eq!(rows.len(), 2, "Expected 2 rows, got: {rows:?}");
        // Row 1: cells "a " and " b "
        assert_eq!(rows[0].len(), 2, "Expected 2 cells in row 1");
        // Row 2: cells "c " and " d "
        assert_eq!(rows[1].len(), 2, "Expected 2 cells in row 2");
    } else {
        panic!("Expected Tabular node, got {:?}", content[0]);
    }
}

/// A row ending with `\tab` but no `\cr` must produce two cells; the trailing
/// whitespace-only cell must not be silently dropped.
#[test]
fn test_tabular_trailing_tab_without_cr() {
    let doc = parse(r#"\details{\tabular{ll}{a \tab }}"#).unwrap();
    let content = &doc.sections[0].content;
    if let RdNode::Tabular { alignment, rows } = &content[0] {
        assert_eq!(alignment, "ll");
        assert_eq!(rows.len(), 1, "Expected 1 row, got: {rows:?}");
        assert_eq!(rows[0].len(), 2, "Expected 2 cells (second is whitespace-only), got: {:?}", rows[0]);
    } else {
        panic!("Expected Tabular node, got {:?}", content[0]);
    }
}

/// In non-code sections (e.g. `\description{}`), bare `{{...}}` remains
/// Rd text grouping: the braces are unwrapped and the inner content is kept.
#[test]
fn test_description_unwraps_grouping_braces() {
    let doc = parse(r#"\description{{grouped text}}"#).unwrap();
    let content = &doc.sections[0].content;
    let text = content
        .iter()
        .filter_map(|n| {
            if let RdNode::Text(s) = n {
                Some(s.as_str())
            } else {
                None
            }
        })
        .collect::<String>();
    assert_eq!(text, "grouped text");
}
