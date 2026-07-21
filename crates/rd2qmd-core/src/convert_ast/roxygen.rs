//! Detection of roxygen2's three-node Markdown fenced-code representation.

use rd_ast::{RdConditionalKind, RdNode, RdPath, RdTag};

use super::{blocks::recover_verbatim, leaf_text::flatten_verbatim_leaves};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoxygenCodeBlock {
    pub(crate) language: Option<String>,
    pub(crate) code: String,
}

pub(crate) fn try_match_roxygen_code_block(nodes: &[RdNode]) -> Option<RoxygenCodeBlock> {
    let [opening, preformatted, closing, ..] = nodes else {
        return None;
    };

    let language = extract_language_from_div(&conditional_out_text(opening)?)?;

    let tagged = preformatted.as_tagged()?;
    if tagged.tag() != &RdTag::Preformatted {
        return None;
    }
    let code = recover_verbatim(tagged.children());

    if !is_closing_div(&conditional_out_text(closing)?) {
        return None;
    }

    Some(RoxygenCodeBlock { language, code })
}

fn conditional_out_text(node: &RdNode) -> Option<String> {
    let base_path = RdPath::new(Vec::new());
    let conditional = node.inspect_conditional(&base_path).ok()??;
    if conditional.kind() != RdConditionalKind::If
        || conditional.format() != "html"
        || conditional.else_branch().is_some()
    {
        return None;
    }

    let [out] = conditional.then_branch() else {
        return None;
    };
    let tagged = out.as_tagged()?;
    (tagged.tag() == &RdTag::Out).then(|| recover_out_text(tagged.children()))
}

fn recover_out_text(nodes: &[RdNode]) -> String {
    flatten_verbatim_leaves(nodes).unwrap_or_else(|error| error.recovered_text().to_owned())
}

/// Recognizes only the exact opening-tag shape roxygen2 emits:
/// `<div class="sourceCode LANG">`, `<div class="sourceCode">`, or the bare
/// `<div class="r">` used for R6 method-usage docs. This deliberately does
/// not parse general HTML — roxygen2 is the sole realistic producer of this
/// string (it only ever reaches here as `\out{}` content immediately
/// preceding a `\preformatted{}`/closing-`\if` pair), so matching its exact
/// grammar closes off the whole class of nested-tag/quoted-attribute tricks
/// a general attribute scanner would have to keep chasing. If roxygen2 ever
/// changes its output shape, this should fall through until the new shape
/// is explicitly supported, not be loosened to parse arbitrary HTML.
fn extract_language_from_div(html: &str) -> Option<Option<String>> {
    let class_value = html
        .trim()
        .strip_prefix(r#"<div class=""#)?
        .strip_suffix(r#"">"#)?;

    // `strip_suffix` only anchors the LAST `">`, so a value containing an
    // earlier `"` (the real attribute-value close) followed by nested
    // markup ending in `">` would otherwise slip through with that markup
    // folded into the "language". A genuine class value never contains
    // quote or angle-bracket characters, so reject anything that does.
    if class_value.contains(['"', '<', '>']) {
        return None;
    }

    let mut tokens = class_value.split_whitespace();
    match (tokens.next(), tokens.next(), tokens.next()) {
        (Some("sourceCode"), None, None) => Some(None),
        (Some("sourceCode"), Some(language), None) => Some(Some(language.to_owned())),
        (Some("r"), None, None) => Some(Some("r".to_owned())),
        _ => None,
    }
}

fn is_closing_div(html: &str) -> bool {
    html.trim() == "</div>"
}

#[cfg(test)]
mod tests {
    use rd2qmd_mdast::{Node, Root, WriterOptions, mdast_to_qmd};

    use super::try_match_roxygen_code_block;
    use crate::convert_ast::blocks::{BlockConversionContext, convert_block_content};
    use crate::convert_ast::inline::LinkResolutionContext;

    fn match_snippet(source: &str) -> Option<super::RoxygenCodeBlock> {
        let parsed = rd_source::parse(source.as_bytes()).unwrap();
        try_match_roxygen_code_block(parsed.document().nodes())
    }

    fn snippet(format: &str, class: &str, code: &str, closing: bool) -> String {
        let mut source =
            format!(r#"\if{{{format}}}{{\out{{<div class="{class}">}}}}\preformatted{{{code}}}"#);
        if closing {
            source.push_str(r#"\if{html}{\out{</div>}}"#);
        }
        source
    }

    #[test]
    fn matches_supported_language_classes() {
        for (class, code, expected_language) in [
            ("sourceCode r", "x <- 1 + 2", Some("r")),
            ("sourceCode python", "print('hello')", Some("python")),
            ("sourceCode", "plain text", None),
            ("r", "hello_r6$new()", Some("r")),
            ("sourceCode yaml", "key: value", Some("yaml")),
            ("sourceCode sql", "SELECT 1", Some("sql")),
        ] {
            let block = match_snippet(&snippet("html", class, code, true))
                .unwrap_or_else(|| panic!("expected class {class:?} to match"));
            assert_eq!(block.language.as_deref(), expected_language);
            assert_eq!(block.code, code);
        }
    }

    #[test]
    fn rejects_non_roxygen_shapes_without_partial_matches() {
        let cases = [
            snippet("latex", "sourceCode r", "code", true),
            r#"\preformatted{code}"#.to_owned(),
            snippet("html", "sourceCode r", "code", false),
            snippet("html", "someOtherClass", "code", true),
            snippet("html", "sourceCodeExtra", "code", true),
            snippet("html", "rSuffix", "code", true),
            r#"\if{html}{\out{<span class="sourceCode r">}}\preformatted{code
}\if{html}{\out{</div>}}"#
                .to_owned(),
            r#"\if{html}{\out{<diverse class="sourceCode r">}}\preformatted{code
}\if{html}{\out{</div>}}"#
                .to_owned(),
            r#"\if{html}{\out{<div data-class="sourceCode r">}}\preformatted{code
}\if{html}{\out{</div>}}"#
                .to_owned(),
            r#"\if{html}{\out{<div><span class="sourceCode r">}}\preformatted{code
}\if{html}{\out{</div>}}"#
                .to_owned(),
            r#"\if{html}{\out{<div title=' class="sourceCode r"'>}}\preformatted{code
}\if{html}{\out{</div>}}"#
                .to_owned(),
            r#"\if{html}{\out{<div class="sourceCode r"><span>">}}\preformatted{code
}\if{html}{\out{</div>}}"#
                .to_owned(),
        ];

        for source in cases {
            assert!(
                match_snippet(&source).is_none(),
                "unexpected match for {source:?}"
            );
        }
    }

    #[test]
    fn parsed_roxygen_block_renders_as_fenced_code() {
        let parsed = rd_source::parse(
            br#"\if{html}{\out{<div class="sourceCode r">}}\preformatted{x <- 1 + 2
}\if{html}{\out{</div>}}"#,
        )
        .unwrap();
        let converted = convert_block_content(
            parsed.document().nodes(),
            &BlockConversionContext {
                links: LinkResolutionContext::default(),
                prefer_ascii_math: false,
                enclosing_heading_depth: 2,
            },
        );

        assert_eq!(
            converted,
            vec![Node::code(Some("r".to_owned()), "x <- 1 + 2\n")]
        );
        assert_eq!(
            mdast_to_qmd(
                &Root::new(converted),
                &WriterOptions {
                    frontmatter: None,
                    quarto_code_blocks: false,
                }
            ),
            "```r\nx <- 1 + 2\n```\n"
        );
    }

    #[test]
    fn parsed_wrong_format_falls_through_existing_conversion() {
        let parsed = rd_source::parse(
            br#"\if{latex}{\out{<div class="sourceCode r">}}\preformatted{x <- 1 + 2
}\if{html}{\out{</div>}}"#,
        )
        .unwrap();
        let converted = convert_block_content(
            parsed.document().nodes(),
            &BlockConversionContext {
                links: LinkResolutionContext::default(),
                prefer_ascii_math: false,
                enclosing_heading_depth: 2,
            },
        );

        assert_eq!(converted, vec![Node::code(None, "x <- 1 + 2\n")]);
    }
}
