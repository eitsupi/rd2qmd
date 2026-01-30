
use super::*;
use rd_parser::parse;
use rd2qmd_mdast::mdast_to_qmd;

#[test]
fn test_simple_conversion() {
    let doc = parse("\\name{test}\n\\title{Test Title}").unwrap();
    let mdast = rd_to_mdast(&doc);

    assert!(!mdast.children.is_empty());
    assert!(matches!(&mdast.children[0], Node::Heading(_)));
}

#[test]
fn test_code_section() {
    let doc = parse("\\title{Test}\n\\usage{foo(x, y)}").unwrap();
    let mdast = rd_to_mdast(&doc);

    // Should have title heading, usage heading, and code block
    assert!(mdast.children.len() >= 2);
    assert!(mdast.children.iter().any(|n| matches!(n, Node::Code(_))));
}

#[test]
fn test_inline_code() {
    let doc = parse("\\title{T}\n\\description{Use \\code{foo}}").unwrap();
    let mdast = rd_to_mdast(&doc);

    // Find the paragraph with inline code
    let has_inline_code = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| matches!(c, Node::InlineCode(_)))
        } else {
            false
        }
    });
    assert!(has_inline_code);
}

#[test]
fn test_normalize_whitespace() {
    // Preserves leading and trailing whitespace
    assert_eq!(normalize_whitespace(" foo"), " foo");
    assert_eq!(normalize_whitespace("foo "), "foo ");
    assert_eq!(normalize_whitespace(" foo "), " foo ");
    // Collapses internal whitespace
    assert_eq!(normalize_whitespace("foo  bar"), "foo bar");
    assert_eq!(normalize_whitespace(" foo  bar "), " foo bar ");
    // Whitespace-only becomes single space
    assert_eq!(normalize_whitespace(" "), " ");
    assert_eq!(normalize_whitespace("   "), " ");
    // Empty stays empty
    assert_eq!(normalize_whitespace(""), "");
}

#[test]
fn test_whitespace_around_inline_code() {
    // Whitespace around \code should be preserved
    let doc = parse("\\title{T}\n\\description{via \\code{x}, \\code{y} text}").unwrap();
    let mdast = rd_to_mdast(&doc);

    // Find paragraph and check the text contains spaces around inline codes
    for node in &mdast.children {
        if let Node::Paragraph(p) = node {
            // Check that we have Text with trailing space before inline code
            let mut found_space_before_code = false;
            for (i, child) in p.children.iter().enumerate() {
                if let Node::Text(t) = child {
                    if t.value.ends_with(' ')
                        && i + 1 < p.children.len()
                        && matches!(p.children[i + 1], Node::InlineCode(_))
                    {
                        found_space_before_code = true;
                    }
                }
            }
            assert!(
                found_space_before_code,
                "Expected whitespace before inline code"
            );
        }
    }
}

#[test]
fn test_list_conversion() {
    let doc = parse("\\title{T}\n\\details{\\itemize{\\item A\\item B}}").unwrap();
    let mdast = rd_to_mdast(&doc);

    assert!(mdast.children.iter().any(|n| matches!(n, Node::List(_))));
}

#[test]
fn test_internal_link_unresolved_becomes_inline_code() {
    // When topic is not in alias_map and no fallback URL, it becomes inline code
    let doc = parse("\\title{T}\n\\description{See \\link{other_func}}").unwrap();
    let options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
        alias_map: None,
        unresolved_link_url: None,
        external_package_urls: None,
        exec_dontrun: false,
        exec_donttest: false,
        quarto_code_blocks: true,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should be inline code, not a link
    let has_inline_code = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| {
                if let Node::InlineCode(ic) = c {
                    ic.value == "other_func"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_inline_code,
        "Expected unresolved link to become inline code"
    );
}

#[test]
fn test_internal_link_with_fallback_url() {
    // When topic is not in alias_map but fallback URL is set, use it
    let doc = parse("\\title{T}\n\\description{See \\link{vector}}").unwrap();
    let options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
        alias_map: None,
        unresolved_link_url: Some("https://rdrr.io/r/base/{topic}.html".to_string()),
        external_package_urls: None,
        exec_dontrun: false,
        exec_donttest: false,
        quarto_code_blocks: true,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should be a link to the fallback URL
    let has_link = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| {
                if let Node::Link(l) = c {
                    l.url == "https://rdrr.io/r/base/vector.html"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_link,
        "Expected fallback URL to be used for unresolved link"
    );
}

#[test]
fn test_internal_link_without_extension() {
    let doc = parse("\\title{T}\n\\description{See \\link{other_func}}").unwrap();
    let options = ConverterOptions::default(); // No link_extension
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should be inline code, not a link
    let has_inline_code = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| {
                if let Node::InlineCode(ic) = c {
                    ic.value == "other_func"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_inline_code,
        "Expected internal link without extension to be inline code"
    );
}

#[test]
fn test_external_link_without_url_becomes_inline_code() {
    let doc = parse("\\title{T}\n\\description{See \\link[dplyr]{filter}}").unwrap();
    let options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
        alias_map: None,
        unresolved_link_url: None,
        external_package_urls: None,
        exec_dontrun: false,
        exec_donttest: false,
        quarto_code_blocks: true,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // External links without URL map should be inline code
    let has_inline_code = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| {
                if let Node::InlineCode(ic) = c {
                    ic.value == "dplyr::filter"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_inline_code,
        "Expected external link without URL to be inline code"
    );
}

#[test]
fn test_external_link_with_url_becomes_hyperlink() {
    use std::collections::HashMap;

    let doc = parse("\\title{T}\n\\description{See \\link[dplyr]{filter}}").unwrap();

    let mut external_urls = HashMap::new();
    external_urls.insert(
        "dplyr".to_string(),
        "https://dplyr.tidyverse.org/reference".to_string(),
    );

    let options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
        alias_map: None,
        unresolved_link_url: None,
        external_package_urls: Some(external_urls),
        exec_dontrun: false,
        exec_donttest: false,
        quarto_code_blocks: true,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // External links with URL map should become hyperlinks
    let has_link = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| {
                if let Node::Link(l) = c {
                    l.url == "https://dplyr.tidyverse.org/reference/filter.html"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_link,
        "Expected external link with URL to become hyperlink"
    );
}

#[test]
fn test_external_link_with_topic_in_package() {
    use std::collections::HashMap;

    // Test \link[rlang:dyn-dots]{text} pattern where pkg:topic is in the package field
    let doc = parse("\\title{T}\n\\description{See \\link[rlang:abort]{abort}}").unwrap();

    let mut external_urls = HashMap::new();
    external_urls.insert(
        "rlang".to_string(),
        "https://rlang.r-lib.org/reference".to_string(),
    );

    let options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
        alias_map: None,
        unresolved_link_url: None,
        external_package_urls: Some(external_urls),
        exec_dontrun: false,
        exec_donttest: false,
        quarto_code_blocks: true,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should use the topic from the pkg:topic format
    let has_link = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| {
                if let Node::Link(l) = c {
                    l.url == "https://rlang.r-lib.org/reference/abort.html"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_link,
        "Expected external link with pkg:topic to resolve topic correctly"
    );
}

#[test]
fn test_alias_resolution() {
    use std::collections::HashMap;

    let doc = parse("\\title{T}\n\\description{See \\link{DataFrame}}").unwrap();

    // Create an alias map: DataFrame -> pl__DataFrame
    let mut alias_map = HashMap::new();
    alias_map.insert("DataFrame".to_string(), "pl__DataFrame".to_string());

    let options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
        alias_map: Some(alias_map),
        unresolved_link_url: None,
        external_package_urls: None,
        exec_dontrun: false,
        exec_donttest: false,
        quarto_code_blocks: true,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Find the paragraph with a link that has resolved alias
    let has_resolved_link = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| {
                if let Node::Link(l) = c {
                    l.url == "pl__DataFrame.qmd"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_resolved_link,
        "Expected alias 'DataFrame' to resolve to 'pl__DataFrame.qmd'"
    );
}

#[test]
fn test_examples_comes_last() {
    // Custom sections should come before Examples (pkgdown convention)
    let rd = r#"
\title{Test}
\description{A test}
\examples{code()}
\section{Custom Section}{Some custom content}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    // Find positions of "Custom Section" and "Examples" headings
    let mut custom_pos = None;
    let mut examples_pos = None;

    for (i, node) in mdast.children.iter().enumerate() {
        if let Node::Heading(h) = node {
            let text: String = h
                .children
                .iter()
                .filter_map(|n| {
                    if let Node::Text(t) = n {
                        Some(t.value.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            if text == "Custom Section" {
                custom_pos = Some(i);
            } else if text == "Examples" {
                examples_pos = Some(i);
            }
        }
    }

    assert!(custom_pos.is_some(), "Custom Section heading not found");
    assert!(examples_pos.is_some(), "Examples heading not found");
    assert!(
        custom_pos.unwrap() < examples_pos.unwrap(),
        "Custom sections should come before Examples"
    );
}

#[test]
fn test_code_wrapping_link_preserves_link() {
    use std::collections::HashMap;

    // Test \code{\link[=alias]{text}} pattern - link should be preserved
    let doc = parse(
        "\\title{T}\n\\description{See \\code{\\link[=as_polars_series]{as_polars_series()}}}",
    )
    .unwrap();

    let mut alias_map = HashMap::new();
    alias_map.insert(
        "as_polars_series".to_string(),
        "as_polars_series".to_string(),
    );

    let options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
        alias_map: Some(alias_map),
        unresolved_link_url: None,
        external_package_urls: None,
        exec_dontrun: false,
        exec_donttest: false,
        quarto_code_blocks: true,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should have a link with inline code as link text
    let has_link_with_code = mdast.children.iter().any(|n| {
        if let Node::Paragraph(p) = n {
            p.children.iter().any(|c| {
                if let Node::Link(l) = c {
                    l.url == "as_polars_series.qmd"
                        && l.children.iter().any(|child| {
                            if let Node::InlineCode(ic) = child {
                                ic.value == "as_polars_series()"
                            } else {
                                false
                            }
                        })
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_link_with_code,
        "Expected \\code{{\\link[=alias]{{text}}}} to produce a link with inline code text"
    );
}

#[test]
fn test_s3_method_comment_in_usage() {
    // Test S3 method with specific class
    let rd = r#"
\title{Test}
\usage{
\method{print}{data.frame}(x, ...)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    // Find the code block and check it contains the S3 method comment
    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(
        code_content.is_some(),
        "Expected a code block in usage section"
    );
    let code = code_content.unwrap();
    assert!(
        code.contains("# S3 method for class 'data.frame'"),
        "Expected S3 method comment for class 'data.frame', got: {}",
        code
    );
    assert!(
        code.contains("print(x, ...)"),
        "Expected function signature, got: {}",
        code
    );
}

#[test]
fn test_s3_default_method_comment_in_usage() {
    // Test S3 default method
    let rd = r#"
\title{Test}
\usage{
\method{print}{default}(x, ...)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(
        code_content.is_some(),
        "Expected a code block in usage section"
    );
    let code = code_content.unwrap();
    assert!(
        code.contains("# Default S3 method"),
        "Expected 'Default S3 method' comment, got: {}",
        code
    );
}

#[test]
fn test_s4_method_comment_in_usage() {
    // Test S4 method
    let rd = r#"
\title{Test}
\usage{
\S4method{show}{MyClass}(object)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(
        code_content.is_some(),
        "Expected a code block in usage section"
    );
    let code = code_content.unwrap();
    assert!(
        code.contains("# S4 method for signature 'MyClass'"),
        "Expected S4 method comment, got: {}",
        code
    );
    assert!(
        code.contains("show(object)"),
        "Expected function signature, got: {}",
        code
    );
}

#[test]
fn test_s4_method_with_multiple_signatures() {
    // Test S4 method with comma-separated signatures
    let rd = r#"
\title{Test}
\usage{
\S4method{coerce}{OldClass,NewClass}(from, to)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(
        code_content.is_some(),
        "Expected a code block in usage section"
    );
    let code = code_content.unwrap();
    assert!(
        code.contains("# S4 method for signature 'OldClass,NewClass'"),
        "Expected S4 method comment with multiple signatures, got: {}",
        code
    );
}

#[test]
fn test_mixed_usage_with_s3_methods() {
    // Test usage with regular function and S3 methods
    let rd = r#"
\title{Test}
\usage{
arrange(.data, ...)

\method{arrange}{data.frame}(.data, ..., .by_group = FALSE)

\method{arrange}{default}(.data, ...)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(
        code_content.is_some(),
        "Expected a code block in usage section"
    );
    let code = code_content.unwrap();

    // Check regular function is present
    assert!(
        code.contains("arrange(.data, ...)"),
        "Expected regular function, got: {}",
        code
    );

    // Check S3 method for data.frame
    assert!(
        code.contains("# S3 method for class 'data.frame'"),
        "Expected S3 method comment for data.frame, got: {}",
        code
    );

    // Check default S3 method
    assert!(
        code.contains("# Default S3 method"),
        "Expected Default S3 method comment, got: {}",
        code
    );
}

#[test]
fn test_s3_method_with_special_class_name() {
    // Test S3 method with special characters in class name
    let rd = r#"
\title{Test}
\usage{
\method{print}{tbl_df}(x, ..., n = NULL, width = NULL)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(
        code_content.is_some(),
        "Expected a code block in usage section"
    );
    let code = code_content.unwrap();
    assert!(
        code.contains("# S3 method for class 'tbl_df'"),
        "Expected S3 method comment with special class name, got: {}",
        code
    );
}

#[test]
fn test_s3_method_with_operator_generic() {
    // Test S3 method for operators like [, [[, $
    let rd = r#"
\title{Test}
\usage{
\method{[}{data.frame}(x, i, j, drop = TRUE)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(
        code_content.is_some(),
        "Expected a code block in usage section"
    );
    let code = code_content.unwrap();
    assert!(
        code.contains("# S3 method for class 'data.frame'"),
        "Expected S3 method comment, got: {}",
        code
    );
    // Infix operators are now formatted naturally: x[i, j, drop = TRUE]
    assert!(
        code.contains("x[i, j, drop = TRUE]"),
        "Expected subscript operator in natural form, got: {}",
        code
    );
}

#[test]
fn test_infix_binary_operators() {
    // Test binary infix operators are formatted naturally
    let rd = r#"
\title{Test}
\usage{
\method{+}{polars_expr}(e1, e2)

\method{-}{polars_expr}(e1, e2)

\method{*}{polars_expr}(e1, e2)

\method{/}{polars_expr}(e1, e2)

\method{^}{polars_expr}(e1, e2)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(code_content.is_some(), "Expected a code block");
    let code = code_content.unwrap();

    // Padded operators (with spaces)
    assert!(
        code.contains("e1 + e2"),
        "Expected 'e1 + e2', got: {}",
        code
    );
    assert!(
        code.contains("e1 - e2"),
        "Expected 'e1 - e2', got: {}",
        code
    );
    assert!(
        code.contains("e1 * e2"),
        "Expected 'e1 * e2', got: {}",
        code
    );
    assert!(
        code.contains("e1 / e2"),
        "Expected 'e1 / e2', got: {}",
        code
    );

    // Unpadded operator (no spaces around ^)
    assert!(code.contains("e1^e2"), "Expected 'e1^e2', got: {}", code);
}

#[test]
fn test_infix_user_defined_operators() {
    // Test user-defined infix operators (%...%)
    let rd = r#"
\title{Test}
\usage{
\method{\%\%}{polars_expr}(e1, e2)

\method{\%/\%}{polars_expr}(e1, e2)

\method{\%>\%}{polars_expr}(lhs, rhs)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(code_content.is_some(), "Expected a code block");
    let code = code_content.unwrap();

    assert!(
        code.contains("e1 %% e2"),
        "Expected 'e1 %% e2', got: {}",
        code
    );
    assert!(
        code.contains("e1 %/% e2"),
        "Expected 'e1 %/% e2', got: {}",
        code
    );
    assert!(
        code.contains("lhs %>% rhs"),
        "Expected 'lhs %>% rhs', got: {}",
        code
    );
}

#[test]
fn test_infix_subscript_operators() {
    // Test subscript operators [, [[, $
    let rd = r#"
\title{Test}
\usage{
\method{[}{data.frame}(x, i, j, drop = TRUE)

\method{[[}{list}(x, i)

\method{$}{env}(x, name)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(code_content.is_some(), "Expected a code block");
    let code = code_content.unwrap();

    assert!(
        code.contains("x[i, j, drop = TRUE]"),
        "Expected 'x[i, j, drop = TRUE]', got: {}",
        code
    );
    assert!(code.contains("x[[i]]"), "Expected 'x[[i]]', got: {}", code);
    assert!(code.contains("x$name"), "Expected 'x$name', got: {}", code);
}

#[test]
fn test_infix_comparison_operators() {
    // Test comparison operators
    let rd = r#"
\title{Test}
\usage{
\method{<}{polars_expr}(e1, e2)

\method{>}{polars_expr}(e1, e2)

\method{==}{polars_expr}(e1, e2)

\method{!=}{polars_expr}(e1, e2)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(code_content.is_some(), "Expected a code block");
    let code = code_content.unwrap();

    assert!(
        code.contains("e1 < e2"),
        "Expected 'e1 < e2', got: {}",
        code
    );
    assert!(
        code.contains("e1 > e2"),
        "Expected 'e1 > e2', got: {}",
        code
    );
    assert!(
        code.contains("e1 == e2"),
        "Expected 'e1 == e2', got: {}",
        code
    );
    assert!(
        code.contains("e1 != e2"),
        "Expected 'e1 != e2', got: {}",
        code
    );
}

#[test]
fn test_s4_method_infix_operator() {
    // Test S4 methods with infix operators
    let rd = r#"
\title{Test}
\usage{
\S4method{+}{MyClass,MyClass}(e1, e2)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(code_content.is_some(), "Expected a code block");
    let code = code_content.unwrap();

    assert!(
        code.contains("# S4 method for signature 'MyClass,MyClass'"),
        "Expected S4 method comment, got: {}",
        code
    );
    assert!(
        code.contains("e1 + e2"),
        "Expected 'e1 + e2', got: {}",
        code
    );
}

#[test]
fn test_non_infix_methods_unchanged() {
    // Regular methods should not be affected
    let rd = r#"
\title{Test}
\usage{
\method{print}{data.frame}(x, ...)

\method{summary}{lm}(object, ...)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);

    let code_content = mdast.children.iter().find_map(|n| {
        if let Node::Code(c) = n {
            Some(c.value.clone())
        } else {
            None
        }
    });

    assert!(code_content.is_some(), "Expected a code block");
    let code = code_content.unwrap();

    assert!(
        code.contains("print(x, ...)"),
        "Expected 'print(x, ...)', got: {}",
        code
    );
    assert!(
        code.contains("summary(object, ...)"),
        "Expected 'summary(object, ...)', got: {}",
        code
    );
}

#[test]
fn test_dontrun_default_not_executable() {
    // By default, dontrun code is shown but not executable
    let rd = r#"
\name{test}
\title{Test}
\examples{
regular_code()
\dontrun{
  slow_code()
}
}
"#;
    let doc = parse(rd).unwrap();
    let options = ConverterOptions::default();
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should have two code blocks
    let code_blocks: Vec<_> = mdast
        .children
        .iter()
        .filter_map(|n| {
            if let Node::Code(c) = n {
                Some(c.clone())
            } else {
                None
            }
        })
        .collect();

    // First block should be executable (meta = "executable")
    assert!(code_blocks.len() >= 2, "Expected at least 2 code blocks");
    assert_eq!(
        code_blocks[0].meta.as_deref(),
        Some("executable"),
        "First block should be executable"
    );
    // Second block (dontrun) should NOT be executable
    assert_ne!(
        code_blocks[1].meta.as_deref(),
        Some("executable"),
        "Dontrun block should not be executable by default"
    );
    assert!(
        code_blocks[1].value.contains("slow_code()"),
        "Dontrun block should contain slow_code()"
    );
}

#[test]
fn test_exec_dontrun_makes_executable() {
    // With exec_dontrun=true, dontrun code becomes executable
    let rd = r#"
\name{test}
\title{Test}
\examples{
\dontrun{
  slow_code()
}
}
"#;
    let doc = parse(rd).unwrap();
    let options = ConverterOptions {
        exec_dontrun: true,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should have one executable code block
    let code_blocks: Vec<_> = mdast
        .children
        .iter()
        .filter_map(|n| {
            if let Node::Code(c) = n {
                Some(c.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(!code_blocks.is_empty(), "Expected at least one code block");
    assert_eq!(
        code_blocks[0].meta.as_deref(),
        Some("executable"),
        "Dontrun block with exec_dontrun=true should be executable"
    );
    assert!(
        code_blocks[0].value.contains("slow_code()"),
        "Block should contain slow_code()"
    );
}

#[test]
fn test_donttest_default_executable() {
    // By default (pkgdown-compatible), donttest code IS executable
    // because \donttest{} means "don't run during testing" but should run normally
    let rd = r#"
\name{test}
\title{Test}
\examples{
\donttest{
  test_code()
}
}
"#;
    let doc = parse(rd).unwrap();
    let options = ConverterOptions::default();
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should have one executable code block
    let code_blocks: Vec<_> = mdast
        .children
        .iter()
        .filter_map(|n| {
            if let Node::Code(c) = n {
                Some(c.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(!code_blocks.is_empty(), "Expected at least one code block");
    assert_eq!(
        code_blocks[0].meta.as_deref(),
        Some("executable"),
        "Donttest block should be executable by default (pkgdown semantics)"
    );
    assert!(
        code_blocks[0].value.contains("test_code()"),
        "Block should contain test_code()"
    );
}

#[test]
fn test_no_exec_donttest_makes_not_executable() {
    // With exec_donttest=false, donttest code is shown but not executable
    let rd = r#"
\name{test}
\title{Test}
\examples{
\donttest{
  test_code()
}
}
"#;
    let doc = parse(rd).unwrap();
    let options = ConverterOptions {
        exec_donttest: false,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);

    // Should have one non-executable code block
    let code_blocks: Vec<_> = mdast
        .children
        .iter()
        .filter_map(|n| {
            if let Node::Code(c) = n {
                Some(c.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(!code_blocks.is_empty(), "Expected at least one code block");
    assert_ne!(
        code_blocks[0].meta.as_deref(),
        Some("executable"),
        "Donttest block with exec_donttest=false should NOT be executable"
    );
    assert!(
        code_blocks[0].value.contains("test_code()"),
        "Block should contain test_code()"
    );
}

#[test]
fn test_arguments_pipe_table_snapshot() {
    let rd = r#"
\name{test}
\title{Test Function}
\arguments{
\item{x}{A simple description.}
\item{data}{Input data. Use \code{NULL} for default.}
}
"#;
    let doc = parse(rd).unwrap();
    let options = ConverterOptions {
        arguments_format: ArgumentsFormat::PipeTable,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());
    insta::assert_snapshot!(qmd);
}

#[test]
fn test_arguments_grid_table_simple_snapshot() {
    let rd = r#"
\name{test}
\title{Test Function}
\arguments{
\item{x}{A simple description.}
\item{data}{Input data. Use \code{NULL} for default.}
}
"#;
    let doc = parse(rd).unwrap();
    let options = ConverterOptions {
        arguments_format: ArgumentsFormat::GridTable,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());
    insta::assert_snapshot!(qmd);
}

#[test]
fn test_arguments_grid_table_with_list_snapshot() {
    let rd = r#"
\name{test}
\title{Test Function}
\arguments{
\item{x}{A simple description.}
\item{opts}{A named list of options:
\itemize{
\item option A
\item option B
\item option C
}}
\item{data}{Input data. Use \code{NULL} for default.}
}
"#;
    let doc = parse(rd).unwrap();
    let options = ConverterOptions {
        arguments_format: ArgumentsFormat::GridTable,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());
    insta::assert_snapshot!(qmd);
}

#[test]
fn test_arguments_grid_table_with_lifecycle_badge_snapshot() {
    // Regression test: lifecycle badge images inside href in grid table cells
    // should preserve the image alt text (e.g., [Experimental], [Deprecated])
    let rd = r#"
\name{test}
\title{Test Function}
\arguments{
\item{engine}{The engine name. One of:
\itemize{
\item \code{"streaming"}: \ifelse{html}{\href{https://lifecycle.r-lib.org/articles/stages.html#experimental}{\figure{lifecycle-experimental.svg}{options: alt='[Experimental]'}}}{\strong{[Experimental]}} Use streaming.
}}
\item{type_coercion}{\ifelse{html}{\href{https://lifecycle.r-lib.org/articles/stages.html#deprecated}{\figure{lifecycle-deprecated.svg}{options: alt='[Deprecated]'}}}{\strong{[Deprecated]}}
Use a flag instead.}
}
"#;
    let doc = parse(rd).unwrap();
    let options = ConverterOptions {
        arguments_format: ArgumentsFormat::GridTable,
        ..Default::default()
    };
    let mdast = rd_to_mdast_with_options(&doc, &options);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());

    // Verify that lifecycle badge alt text is preserved
    assert!(
        qmd.contains("[![[Experimental]]"),
        "Expected lifecycle badge with [Experimental] alt text"
    );
    assert!(
        qmd.contains("[![[Deprecated]]"),
        "Expected lifecycle badge with [Deprecated] alt text"
    );

    insta::assert_snapshot!(qmd);
}

// ============================================================================
// Integration tests for \figure tag conversion
// ============================================================================

#[test]
fn test_figure_simple_form_alt_text() {
    // Simple form (form 2): \figure{filename}{alternate text}
    // The second argument should be used directly as alt text
    let rd = r#"
\name{test}
\title{Test}
\description{
See the logo: \figure{Rlogo.svg}{R logo}
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());

    // Should contain ![R logo](Rlogo.svg)
    assert!(
        qmd.contains("![R logo](Rlogo.svg)"),
        "Expected simple form alt text 'R logo', got:\n{qmd}"
    );
}

#[test]
fn test_figure_expert_form_alt_text() {
    // Expert form (form 3): \figure{filename}{options: alt="..."}
    let rd = r#"
\name{test}
\title{Test}
\description{
See the logo: \figure{Rlogo.svg}{options: width=100 alt="R logo image"}
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());

    // Should contain ![R logo image](Rlogo.svg)
    assert!(
        qmd.contains("![R logo image](Rlogo.svg)"),
        "Expected expert form alt text 'R logo image', got:\n{qmd}"
    );
}

#[test]
fn test_figure_expert_form_no_alt_fallback_to_filename() {
    // Expert form without alt attribute should fall back to filename
    let rd = r#"
\name{test}
\title{Test}
\description{
See: \figure{diagram.png}{options: width=100}
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());

    // Should contain ![diagram.png](diagram.png) - filename as fallback
    assert!(
        qmd.contains("![diagram.png](diagram.png)"),
        "Expected filename fallback 'diagram.png', got:\n{qmd}"
    );
}

#[test]
fn test_figure_no_second_arg_fallback_to_filename() {
    // Form 1: \figure{filename} - no second argument, filename as alt
    let rd = r#"
\name{test}
\title{Test}
\description{
See: \figure{diagram.png}
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());

    // Should contain ![diagram.png](diagram.png) - filename as fallback
    assert!(
        qmd.contains("![diagram.png](diagram.png)"),
        "Expected filename fallback 'diagram.png', got:\n{qmd}"
    );
}

// ============================================================================
// Unit tests for extract_alt_from_attrs
// ============================================================================

/// Tests for `extract_alt_from_attrs` function which extracts alt text from
/// HTML attributes string (Expert form only).
///
/// Note: The parser now distinguishes between simple form and expert form.
/// - Simple form (FigureOptions::AltText): entire string is alt text
/// - Expert form (FigureOptions::ExpertOptions): "options:" prefix is stripped by parser
///
/// This function only handles the expert form where we need to parse HTML attributes.

#[test]
fn test_extract_alt_from_attrs_double_quotes() {
    // Expert form with double quotes (official documentation example)
    // Note: "options:" prefix is already stripped by parser
    assert_eq!(
        Converter::extract_alt_from_attrs(r#"width=100 alt="R logo""#),
        Some("R logo".to_string())
    );
    assert_eq!(
        Converter::extract_alt_from_attrs(r#"alt="[Deprecated]""#),
        Some("[Deprecated]".to_string())
    );
}

#[test]
fn test_extract_alt_from_attrs_single_quotes() {
    // Expert form with single quotes (lifecycle badge style)
    assert_eq!(
        Converter::extract_alt_from_attrs("alt='[Deprecated]'"),
        Some("[Deprecated]".to_string())
    );
    assert_eq!(
        Converter::extract_alt_from_attrs("alt='[Experimental]'"),
        Some("[Experimental]".to_string())
    );
}

#[test]
fn test_extract_alt_from_attrs_no_alt() {
    // Expert form without alt attribute - should return None (caller uses filename)
    assert_eq!(Converter::extract_alt_from_attrs("width=100"), None);
    assert_eq!(Converter::extract_alt_from_attrs("width=50 height=30"), None);
}

#[test]
fn test_extract_alt_from_attrs_multiple_attributes() {
    // Expert form with multiple attributes
    assert_eq!(
        Converter::extract_alt_from_attrs(r#"width=100 alt="Description" height=50"#),
        Some("Description".to_string())
    );
    assert_eq!(
        Converter::extract_alt_from_attrs("class='badge' alt='[Superseded]' style='margin:0'"),
        Some("[Superseded]".to_string())
    );
}

#[test]
fn test_extract_alt_from_attrs_special_characters() {
    // Alt text with special characters
    assert_eq!(
        Converter::extract_alt_from_attrs("alt='[Experimental - β version]'"),
        Some("[Experimental - β version]".to_string())
    );
    assert_eq!(
        Converter::extract_alt_from_attrs(r#"alt="A & B < C""#),
        Some("A & B < C".to_string())
    );
}

#[test]
fn test_extract_alt_from_attrs_empty_alt() {
    // Empty alt attribute
    assert_eq!(
        Converter::extract_alt_from_attrs("alt=''"),
        Some("".to_string())
    );
    assert_eq!(
        Converter::extract_alt_from_attrs(r#"alt="""#),
        Some("".to_string())
    );
}

#[test]
fn test_extract_alt_from_attrs_empty_string() {
    // Empty string should return None
    assert_eq!(Converter::extract_alt_from_attrs(""), None);
}

// ============================================================================
// Roxygen2 markdown code block tests
// ============================================================================

#[cfg(feature = "roxygen")]
#[test]
fn test_roxygen_code_block_conversion_snapshot() {
    // Test roxygen2 markdown code block pattern:
    // \if{html}{\out{<div class="sourceCode r">}}\preformatted{...}\if{html}{\out{</div>}}
    let rd = r#"
\name{test}
\title{Roxygen Code Block Test}
\description{
Here is an R code block from roxygen2:
\if{html}{\out{<div class="sourceCode r">}}\preformatted{x <- 1 + 2
y <- x * 3
print(y)
}\if{html}{\out{</div>}}

And some more text after the code block.

Python code block:
\if{html}{\out{<div class="sourceCode python">}}\preformatted{def greet(name):
    return f"Hello, {name}!"
}\if{html}{\out{</div>}}

Code block without language:
\if{html}{\out{<div class="sourceCode">}}\preformatted{plain text
block without language
}\if{html}{\out{</div>}}
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());
    insta::assert_snapshot!(qmd);
}

#[cfg(feature = "roxygen")]
#[test]
fn test_roxygen_code_block_with_backticks_in_content() {
    // Test that code containing backticks gets proper fence length
    let rd = r#"
\name{test}
\title{Test}
\description{
\if{html}{\out{<div class="sourceCode r">}}\preformatted{# Code with backticks
x <- "`value`"
y <- "``nested``"
}\if{html}{\out{</div>}}
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());

    // The fence should be longer than the longest backtick run in the content
    assert!(
        qmd.contains("```"),
        "Expected code fence with backticks, got:\n{qmd}"
    );
    insta::assert_snapshot!(qmd);
}

// ========================================================================
// Tests for new tags: \doi, \dontdiff, \S3method, \linkS4class
// ========================================================================

#[test]
fn test_doi_tag() {
    let rd = r#"
\name{test}
\title{DOI Test}
\description{
See the paper at \doi{10.1234/example.2024}.
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());
    insta::assert_snapshot!(qmd);
}

#[test]
fn test_link_s4class_tag() {
    let rd = r#"
\name{test}
\title{LinkS4class Test}
\description{
See \linkS4class{MyClass} and \linkS4class[methods]{representation}.
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());
    insta::assert_snapshot!(qmd);
}

#[test]
fn test_s3method_tag() {
    let rd = r#"
\name{test}
\title{S3method Test}
\usage{
\S3method{print}{myclass}(x, ...)
\S3method{summary}{default}(object)
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());
    insta::assert_snapshot!(qmd);
}

#[test]
fn test_dontdiff_in_examples() {
    let rd = r#"
\name{test}
\title{Dontdiff Test}
\examples{
x <- 1
\dontdiff{
# Output varies - don't diff
print(Sys.time())
}
y <- 2
}
"#;
    let doc = parse(rd).unwrap();
    let mdast = rd_to_mdast(&doc);
    let qmd = mdast_to_qmd(&mdast, &rd2qmd_mdast::WriterOptions::default());
    insta::assert_snapshot!(qmd);
}
