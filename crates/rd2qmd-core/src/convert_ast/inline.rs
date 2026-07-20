//! Conversion of primitive inline rd_ast nodes to mdast nodes.

use rd_ast::{RdEquationDisplay, RdInlineSpanKind, RdNode, RdPath, RdTag};
use rd2qmd_mdast::{Html, Image, Node};

use super::leaf_text::{flatten_prose_leaves, flatten_verbatim_leaves};

/// Convert the primitive inline nodes currently supported by the AST migration.
pub(crate) fn convert_inline_node(node: &RdNode) -> Option<Node> {
    match node {
        RdNode::Text(text) => return Some(Node::text(text.clone())),
        RdNode::RCode(code) | RdNode::Verb(code) => return Some(Node::inline_code(code.clone())),
        _ => {}
    }

    // Real source-path tracking is deferred until a later migration phase.
    let base_path = RdPath::new(Vec::new());

    if let Some(span) = node.inline_span(&base_path) {
        return convert_inline_span(span.kind(), span.body());
    }

    if let Some(symbol) = node.text_symbol(&base_path) {
        return Some(Node::text(symbol.fallback_text().to_owned()));
    }

    let tagged = node.as_tagged()?;
    match tagged.tag() {
        RdTag::Cr if tagged.option().is_none() && tagged.children().is_empty() => Some(Node::Break),
        RdTag::Enc => node
            .enc(&base_path)
            .map(|encoding| Node::text(prose_text(encoding.encoded()))),
        RdTag::Eqn | RdTag::Deqn => tagged.inspect_equation(&base_path).ok().map(|equation| {
            let latex = prose_text(equation.latex());
            match equation.display() {
                RdEquationDisplay::Inline => Node::inline_math(latex),
                RdEquationDisplay::Block => Node::math(latex),
                _ => unreachable!("all equation display kinds are handled"),
            }
        }),
        RdTag::Figure => node.figure(&base_path).map(|figure| {
            let file = figure.file();
            let alt = figure
                .second()
                .and_then(|second| {
                    second
                        .alt_text()
                        .map(str::to_owned)
                        .or_else(|| second.option_attributes().and_then(extract_alt_from_attrs))
                })
                .unwrap_or_else(|| file.to_owned());
            Node::Image(Image {
                url: file.to_owned(),
                title: None,
                alt,
            })
        }),
        RdTag::Out => Some(Node::Html(Html {
            value: verbatim_text(tagged.children()),
        })),
        _ => None,
    }
}

/// Convert all supported primitive inline nodes, skipping out-of-scope nodes.
pub(crate) fn convert_inline_nodes(nodes: &[RdNode]) -> Vec<Node> {
    let mut converted = Vec::new();
    for node in nodes {
        if let RdNode::Group(group) = node {
            converted.extend(convert_inline_nodes(group.children()));
        } else if let Some(node) = convert_inline_node(node) {
            converted.push(node);
        }
    }
    converted
}

/// Extract trimmed plain text from an mdast node sequence.
pub(crate) fn extract_plain_text(nodes: &[Node]) -> String {
    fn append_node_text(node: &Node, text: &mut String) {
        match node {
            Node::Heading(node) => append_children_text(&node.children, text),
            Node::Paragraph(node) => append_children_text(&node.children, text),
            Node::ThematicBreak => {}
            Node::Blockquote(node) => append_children_text(&node.children, text),
            Node::List(node) => append_children_text(&node.children, text),
            Node::ListItem(node) => append_children_text(&node.children, text),
            Node::Code(node) => text.push_str(&node.value),
            Node::Table(node) => append_children_text(&node.children, text),
            Node::TableRow(node) => append_children_text(&node.children, text),
            Node::TableCell(node) => append_children_text(&node.children, text),
            Node::DefinitionList(node) => append_children_text(&node.children, text),
            Node::DefinitionTerm(node) => append_children_text(&node.children, text),
            Node::DefinitionDescription(node) => append_children_text(&node.children, text),
            Node::Text(node) => text.push_str(&node.value),
            Node::Emphasis(node) => append_children_text(&node.children, text),
            Node::Strong(node) => append_children_text(&node.children, text),
            Node::InlineCode(node) => text.push_str(&node.value),
            Node::Break => {}
            Node::Link(node) => append_children_text(&node.children, text),
            Node::Image(node) => text.push_str(&node.alt),
            Node::Math(node) => text.push_str(&node.value),
            Node::InlineMath(node) => text.push_str(&node.value),
            Node::Html(node) => text.push_str(&node.value),
        }
    }

    fn append_children_text(nodes: &[Node], text: &mut String) {
        for node in nodes {
            append_node_text(node, text);
        }
    }

    let mut text = String::new();
    append_children_text(nodes, &mut text);
    text.trim().to_owned()
}

fn convert_inline_span(kind: RdInlineSpanKind, body: &[RdNode]) -> Option<Node> {
    let node = match kind {
        RdInlineSpanKind::Emph | RdInlineSpanKind::Dfn => {
            Node::emphasis(convert_inline_nodes(body))
        }
        RdInlineSpanKind::Strong | RdInlineSpanKind::Bold => {
            Node::strong(convert_inline_nodes(body))
        }
        RdInlineSpanKind::Code
        | RdInlineSpanKind::Samp
        | RdInlineSpanKind::File
        | RdInlineSpanKind::Kbd
        | RdInlineSpanKind::Option
        | RdInlineSpanKind::Command
        | RdInlineSpanKind::Env => Node::inline_code(prose_text(body)),
        RdInlineSpanKind::Verb => Node::inline_code(verbatim_text(body)),
        RdInlineSpanKind::Var | RdInlineSpanKind::Cite => {
            Node::emphasis(vec![Node::text(prose_text(body))])
        }
        RdInlineSpanKind::Acronym | RdInlineSpanKind::Abbr | RdInlineSpanKind::Special => {
            Node::text(prose_text(body))
        }
        RdInlineSpanKind::SQuote => Node::text(format!("'{}'", prose_text(body))),
        RdInlineSpanKind::DQuote => Node::text(format!("\"{}\"", prose_text(body))),
        RdInlineSpanKind::Url => {
            let url = prose_text(body);
            Node::link(url.clone(), vec![Node::text(url)])
        }
        RdInlineSpanKind::Email => {
            let email = prose_text(body);
            Node::link(format!("mailto:{email}"), vec![Node::text(email)])
        }
        RdInlineSpanKind::Pkg => Node::strong(vec![Node::text(prose_text(body))]),
        _ => return None,
    };
    Some(node)
}

fn prose_text(nodes: &[RdNode]) -> String {
    flatten_prose_leaves(nodes).unwrap_or_else(|error| error.recovered_text().to_owned())
}

fn verbatim_text(nodes: &[RdNode]) -> String {
    flatten_verbatim_leaves(nodes).unwrap_or_else(|error| error.recovered_text().to_owned())
}

fn extract_alt_from_attrs(attributes: &str) -> Option<String> {
    for (prefix, quote) in [("alt='", '\''), ("alt=\"", '"')] {
        if let Some((_, value)) = attributes.split_once(prefix)
            && let Some(end) = value.find(quote)
        {
            return Some(value[..end].to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use rd_ast::{RdNode, RdTag};
    use rd2qmd_mdast::Node;

    use super::{convert_inline_node, convert_inline_nodes, extract_plain_text};

    fn text(value: &str) -> RdNode {
        RdNode::Text(value.to_owned())
    }

    fn verb(value: &str) -> RdNode {
        RdNode::Verb(value.to_owned())
    }

    fn tagged(tag: RdTag, children: Vec<RdNode>) -> RdNode {
        RdNode::tagged(tag, None, children)
    }

    fn span(tag: RdTag, value: &str) -> RdNode {
        tagged(tag, vec![text(value)])
    }

    fn group(nodes: Vec<RdNode>) -> RdNode {
        RdNode::group(nodes)
    }

    fn figure(file: &str, second: Option<&str>) -> RdNode {
        let mut children = vec![group(vec![verb(file)])];
        if let Some(second) = second {
            children.push(group(vec![verb(second)]));
        }
        tagged(RdTag::Figure, children)
    }

    #[test]
    fn recursively_converts_nested_emphasis_and_strong() {
        let node = tagged(
            RdTag::Strong,
            vec![text("a "), tagged(RdTag::Emph, vec![text("b")]), text(" c")],
        );

        assert_eq!(
            convert_inline_node(&node),
            Some(Node::strong(vec![
                Node::text("a "),
                Node::emphasis(vec![Node::text("b")]),
                Node::text(" c"),
            ]))
        );
    }

    #[test]
    fn converts_code_like_and_plain_text_spans() {
        let nodes = vec![
            span(RdTag::Code, "f(x)"),
            span(RdTag::Samp, "sample"),
            span(RdTag::Kbd, "Ctrl-C"),
            span(RdTag::Var, "x"),
            span(RdTag::Cite, "Reference"),
            span(RdTag::Acronym, "API"),
            span(RdTag::SQuote, "single"),
            span(RdTag::DQuote, "double"),
        ];

        assert_eq!(
            convert_inline_nodes(&nodes),
            vec![
                Node::inline_code("f(x)"),
                Node::inline_code("sample"),
                Node::inline_code("Ctrl-C"),
                Node::emphasis(vec![Node::text("x")]),
                Node::emphasis(vec![Node::text("Reference")]),
                Node::text("API"),
                Node::text("'single'"),
                Node::text("\"double\""),
            ]
        );
    }

    #[test]
    fn converts_bold_special_and_verbatim_macro_conservatively() {
        let nodes = vec![
            span(RdTag::Bold, "bold"),
            span(RdTag::Special, "raw-ish"),
            tagged(RdTag::Verb, vec![verb("a\\b")]),
        ];

        assert_eq!(
            convert_inline_nodes(&nodes),
            vec![
                Node::strong(vec![Node::text("bold")]),
                Node::text("raw-ish"),
                Node::inline_code("a\\b"),
            ]
        );
    }

    #[test]
    fn converts_url_email_and_package_spans() {
        let nodes = vec![
            span(RdTag::Url, "https://example.com"),
            span(RdTag::Email, "me@example.com"),
            span(RdTag::Pkg, "stats"),
        ];

        assert_eq!(
            convert_inline_nodes(&nodes),
            vec![
                Node::link(
                    "https://example.com",
                    vec![Node::text("https://example.com")],
                ),
                Node::link("mailto:me@example.com", vec![Node::text("me@example.com")]),
                Node::strong(vec![Node::text("stats")]),
            ]
        );
    }

    #[test]
    fn converts_text_symbol_line_break_and_encoding() {
        let nodes = vec![
            tagged(RdTag::R, vec![]),
            tagged(RdTag::Cr, vec![]),
            tagged(
                RdTag::Enc,
                vec![group(vec![text("café")]), group(vec![text("cafe")])],
            ),
        ];

        assert_eq!(
            convert_inline_nodes(&nodes),
            vec![Node::text("R"), Node::Break, Node::text("café")]
        );
    }

    #[test]
    fn converts_inline_and_block_equations_using_latex() {
        let nodes = vec![
            tagged(
                RdTag::Eqn,
                vec![group(vec![text("x^2")]), group(vec![text("x squared")])],
            ),
            tagged(
                RdTag::Deqn,
                vec![group(vec![text("x = y")]), group(vec![text("x equals y")])],
            ),
        ];

        assert_eq!(
            convert_inline_nodes(&nodes),
            vec![Node::inline_math("x^2"), Node::math("x = y")]
        );
    }

    #[test]
    fn skips_malformed_equations() {
        let malformed = tagged(RdTag::Eqn, vec![text("not grouped")]);
        assert_eq!(convert_inline_node(&malformed), None);
    }

    #[test]
    fn converts_figure_alt_text_options_and_filename_fallback() {
        let nodes = vec![
            figure("plot.png", Some("A plot")),
            figure(
                "options.png",
                Some("options: width='50%' alt=\"Options plot\" class='figure'"),
            ),
            figure("fallback.png", None),
        ];

        assert_eq!(
            convert_inline_nodes(&nodes),
            vec![
                Node::image("plot.png", "A plot"),
                Node::image("options.png", "Options plot"),
                Node::image("fallback.png", "fallback.png"),
            ]
        );
    }

    #[test]
    fn converts_raw_out_to_html() {
        let node = tagged(RdTag::Out, vec![verb("<span>raw</span>")]);
        assert_eq!(
            convert_inline_node(&node),
            Some(Node::html("<span>raw</span>"))
        );
    }

    #[test]
    fn skips_nodes_outside_this_substep() {
        let link = tagged(RdTag::Link, vec![text("topic")]);
        assert_eq!(convert_inline_node(&link), None);
    }

    #[test]
    fn extracts_plain_text_from_nested_mdast_nodes() {
        let nodes = vec![
            Node::text("  plain "),
            Node::emphasis(vec![
                Node::text("emphasized "),
                Node::strong(vec![Node::text("strong")]),
            ]),
            Node::inline_code(" code "),
            Node::image("plot.png", "plot alt"),
        ];

        assert_eq!(
            extract_plain_text(&nodes),
            "plain emphasized strong code plot alt"
        );
    }
}
