//! Conversion of inline rd_ast nodes to mdast nodes.

use std::collections::HashMap;

use rd_ast::{
    RdConditionalKind, RdInlineSpanKind, RdLinkDestination, RdLinkTopic, RdNode, RdPath, RdTag,
};
use rd2qmd_mdast::{Html, Image, Node};

use super::leaf_text::flatten_verbatim_leaves;

/// Borrowed configuration used to resolve Rd links without cloning converter options.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LinkResolutionContext<'a> {
    pub(crate) internal_link_url: Option<&'a str>,
    pub(crate) unqualified_link_url: Option<&'a str>,
    pub(crate) external_link_url: Option<&'a str>,
    pub(crate) alias_map: Option<&'a HashMap<String, String>>,
    pub(crate) package_urls: Option<&'a HashMap<String, String>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct InlineConversionContext<'a> {
    pub(crate) links: LinkResolutionContext<'a>,
    pub(crate) include_html_output: bool,
    pub(crate) prefer_ascii_math: bool,
}

/// Replace equation line endings for inline-only output.
///
/// Accepted limitation: flattening LaTeX line endings is not TeX-comment-aware.
/// An unescaped `%` may therefore comment out content that originally followed
/// on a later line; `\%` is a literal percent. Preserving TeX tokenization while
/// producing single-line output would require TeX-aware parsing.
fn single_line_equation_text(value: &str) -> String {
    super::blocks::replace_line_endings_with_space(value)
}

/// Convert the inline nodes currently supported by the AST migration.
pub(crate) fn convert_inline_node(
    node: &RdNode,
    context: &InlineConversionContext<'_>,
) -> Option<Node> {
    match node {
        RdNode::Text(text) => return Some(Node::text(normalize_whitespace(text))),
        RdNode::RCode(code) | RdNode::Verb(code) => return Some(Node::inline_code(code.clone())),
        _ => {}
    }

    // Real source-path tracking is deferred until a later migration phase.
    let base_path = RdPath::new(Vec::new());

    if let Some(span) = node.inline_span(&base_path) {
        return convert_inline_span(span.kind(), span.body(), context);
    }

    if let Some(symbol) = node.text_symbol(&base_path) {
        return Some(Node::text(symbol.fallback_text().to_owned()));
    }

    let tagged = node.as_tagged()?;
    match tagged.tag() {
        RdTag::Cr if tagged.option().is_none() && tagged.children().is_empty() => Some(Node::Break),
        RdTag::Enc => node
            .enc(&base_path)
            .map(|encoding| Node::text(prose_text(encoding.encoded(), context))),
        // convert_inline_node's result can be nested inside Paragraph,
        // Emphasis, Strong, or Link children, where a block node
        // (Node::Code, Node::Math) is structurally unsafe for the writer
        // (mdast.rs treats them as block nodes; the writer's line-start
        // tracking isn't reliable across an inline/block transition). So
        // \deqn -- a display equation -- reached through this path always
        // degrades to its inline representation, regardless of
        // RdEquationDisplay; only the block-scan path (blocks.rs's
        // standalone Deqn handling) may produce a true block-level
        // equation.
        RdTag::Eqn | RdTag::Deqn => tagged.inspect_equation(&base_path).ok().map(|equation| {
            if context.prefer_ascii_math
                && let Some(ascii) = equation.ascii()
            {
                let ascii = super::blocks::recover_verbatim(ascii);
                let ascii = single_line_equation_text(&ascii);
                if !ascii.trim().is_empty() {
                    return Node::inline_code(ascii);
                }
            }
            let latex = prose_text(equation.latex(), context);
            Node::inline_math(single_line_equation_text(&latex))
        }),
        RdTag::If | RdTag::IfElse => {
            let conditional = node.inspect_conditional(&base_path).ok().flatten()?;
            let format = conditional.format();

            let branch = match conditional.kind() {
                RdConditionalKind::If => {
                    let include =
                        format == "text" || (format == "html" && context.include_html_output);

                    if !include {
                        return None;
                    }

                    conditional.then_branch()
                }
                RdConditionalKind::IfElse => {
                    if format == "html" || format == "text" {
                        conditional.then_branch()
                    } else {
                        conditional.else_branch()?
                    }
                }
                _ => unreachable!("all conditional kinds are handled"),
            };

            collapse_inline_nodes(convert_inline_nodes(branch, context))
        }
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
        RdTag::Href => tagged.inspect_href(&base_path).ok().map(|href| {
            Node::link(
                prose_text(href.url(), context),
                convert_inline_nodes(href.display(), context),
            )
        }),
        RdTag::Link => tagged
            .inspect_link(&base_path)
            .ok()
            .and_then(|link| convert_link(&link, context)),
        RdTag::LinkS4Class => convert_s4_class_link(node, &base_path, &context.links),
        RdTag::Sexpr => tagged
            .inspect_sexpr(&base_path)
            .ok()
            .map(|sexpr| Node::inline_code(sexpr.code().to_owned())),
        RdTag::Doi
            if tagged.option().is_none() && matches!(tagged.children(), [RdNode::Text(_)]) =>
        {
            let [RdNode::Text(id)] = tagged.children() else {
                unreachable!("DOI shape was checked above")
            };
            Some(Node::link(
                format!("https://doi.org/{id}"),
                vec![Node::text(format!("doi:{id}"))],
            ))
        }
        // \method/\S3method/\S4method are normally consumed by the \usage
        // block scanner (code.rs), which renders them as a header comment
        // plus the generic call. Outside \usage -- unusual but valid Rd --
        // this mirrors the legacy converter's prose fallback: just the bare
        // generic name as a call, e.g. `print()`.
        RdTag::Method | RdTag::S3Method | RdTag::S4Method => node
            .method(&base_path)
            .map(|method| Node::text(format!("{}()", method.generic()))),
        _ => None,
    }
}

/// Convert a prose text leaf with Markdown-safe whitespace normalization.
pub(super) fn convert_text(text: &str) -> Node {
    Node::text(normalize_whitespace(text))
}

/// Convert all supported inline nodes, skipping out-of-scope nodes.
pub(crate) fn convert_inline_nodes(
    nodes: &[RdNode],
    context: &InlineConversionContext<'_>,
) -> Vec<Node> {
    let mut converted = Vec::new();
    for node in nodes {
        if let RdNode::Group(group) = node {
            converted.extend(convert_inline_nodes(group.children(), context));
        } else if let RdNode::Raw(raw) = node {
            converted.extend(convert_inline_nodes(raw.children(), context));
        } else if let Some(tagged) = node.as_tagged()
            && matches!(tagged.tag(), RdTag::Unknown(_))
        {
            converted.extend(convert_inline_nodes(tagged.children(), context));
        } else if let Some(node) = convert_inline_node(node, context) {
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

fn convert_inline_span(
    kind: RdInlineSpanKind,
    body: &[RdNode],
    context: &InlineConversionContext<'_>,
) -> Option<Node> {
    let node = match kind {
        RdInlineSpanKind::Emph | RdInlineSpanKind::Dfn => {
            Node::emphasis(convert_inline_nodes(body, context))
        }
        RdInlineSpanKind::Strong | RdInlineSpanKind::Bold => {
            Node::strong(convert_inline_nodes(body, context))
        }
        RdInlineSpanKind::Code => {
            if let [node] = body
                && node
                    .as_tagged()
                    .is_some_and(|tagged| tagged.tag() == &RdTag::Link)
            {
                return convert_inline_node(node, context);
            }
            Node::inline_code(prose_text(body, context))
        }
        RdInlineSpanKind::Samp
        | RdInlineSpanKind::File
        | RdInlineSpanKind::Kbd
        | RdInlineSpanKind::Option
        | RdInlineSpanKind::Command
        | RdInlineSpanKind::Env => Node::inline_code(prose_text(body, context)),
        RdInlineSpanKind::Verb => Node::inline_code(verbatim_text(body)),
        RdInlineSpanKind::Var | RdInlineSpanKind::Cite => {
            Node::emphasis(vec![Node::text(prose_text(body, context))])
        }
        RdInlineSpanKind::Acronym | RdInlineSpanKind::Abbr | RdInlineSpanKind::Special => {
            Node::text(prose_text(body, context))
        }
        RdInlineSpanKind::SQuote => Node::text(format!("'{}'", prose_text(body, context))),
        RdInlineSpanKind::DQuote => Node::text(format!("\"{}\"", prose_text(body, context))),
        RdInlineSpanKind::Url => {
            let url = prose_text(body, context);
            Node::link(url.clone(), vec![Node::text(url)])
        }
        RdInlineSpanKind::Email => {
            let email = prose_text(body, context);
            Node::link(format!("mailto:{email}"), vec![Node::text(email)])
        }
        RdInlineSpanKind::Pkg => Node::strong(vec![Node::text(prose_text(body, context))]),
        _ => return None,
    };
    Some(node)
}

fn prose_text(nodes: &[RdNode], context: &InlineConversionContext<'_>) -> String {
    extract_plain_text(&convert_inline_nodes(nodes, context))
}

fn convert_link(link: &rd_ast::RdLink<'_>, context: &InlineConversionContext<'_>) -> Option<Node> {
    match link.destination() {
        RdLinkDestination::DisplayText { nodes } => {
            let topic = prose_text(nodes, context);
            Some(resolve_unqualified_link(
                &topic,
                topic.clone(),
                &context.links,
            ))
        }
        RdLinkDestination::Explicit { topic } => Some(resolve_unqualified_link(
            topic,
            prose_text(link.display(), context),
            &context.links,
        )),
        RdLinkDestination::Package { package, topic } => match topic {
            RdLinkTopic::Explicit(topic) => Some(resolve_qualified_link(
                package,
                topic,
                prose_text(link.display(), context),
                &context.links,
            )),
            RdLinkTopic::DisplayText(nodes) => {
                let topic = prose_text(nodes, context);
                Some(resolve_qualified_link(
                    package,
                    &topic,
                    format!("{package}::{topic}"),
                    &context.links,
                ))
            }
            _ => None,
        },
        _ => None,
    }
}

fn collapse_inline_nodes(nodes: Vec<Node>) -> Option<Node> {
    if nodes.len() == 1 {
        nodes.into_iter().next()
    } else {
        Some(Node::paragraph(nodes))
    }
}

fn convert_s4_class_link(
    node: &RdNode,
    base_path: &RdPath,
    context: &LinkResolutionContext<'_>,
) -> Option<Node> {
    let link = node.s4_class_link(base_path)?;
    let classname = link.class_text()?;
    let topic = format!("{classname}-class");
    match link.package() {
        Some(_) => {
            let package = link.package_text()?;
            Some(resolve_qualified_link(
                &package,
                &topic,
                format!("{package}::{topic}"),
                context,
            ))
        }
        None => Some(resolve_unqualified_link(
            &topic,
            format!("{classname}-class"),
            context,
        )),
    }
}

fn resolve_qualified_link(
    package: &str,
    topic: &str,
    display: String,
    context: &LinkResolutionContext<'_>,
) -> Node {
    if let Some(template) = context
        .package_urls
        .and_then(|package_urls| package_urls.get(package))
    {
        Node::link(
            template.replace("{topic}", topic),
            vec![Node::inline_code(display)],
        )
    } else if let Some(template) = context.external_link_url {
        Node::link(
            template
                .replace("{package}", package)
                .replace("{topic}", topic),
            vec![Node::inline_code(display)],
        )
    } else {
        Node::inline_code(display)
    }
}

fn resolve_unqualified_link(
    topic: &str,
    display: String,
    context: &LinkResolutionContext<'_>,
) -> Node {
    if let Some(target_file) = context.alias_map.and_then(|alias_map| alias_map.get(topic)) {
        if let Some(template) = context.internal_link_url {
            Node::link(
                template
                    .replace("{file}", target_file)
                    .replace("{topic}", topic),
                vec![Node::inline_code(display)],
            )
        } else {
            Node::inline_code(display)
        }
    } else if let Some(template) = context.unqualified_link_url {
        Node::link(
            template.replace("{topic}", topic),
            vec![Node::inline_code(display)],
        )
    } else {
        Node::inline_code(display)
    }
}

fn verbatim_text(nodes: &[RdNode]) -> String {
    flatten_verbatim_leaves(nodes).unwrap_or_else(|error| error.recovered_text().to_owned())
}

fn normalize_whitespace(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let has_leading = text.chars().next().is_some_and(char::is_whitespace);
    let has_trailing = text.chars().next_back().is_some_and(char::is_whitespace);
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if normalized.is_empty() {
        return " ".to_owned();
    }

    let mut result = String::new();
    if has_leading {
        result.push(' ');
    }
    result.push_str(&normalized);
    if has_trailing {
        result.push(' ');
    }
    result
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
    use std::collections::HashMap;

    use rd_ast::{RdNode, RdTag, producer};
    use rd2qmd_mdast::{Node, Root, WriterOptions, mdast_to_qmd};

    use super::{
        InlineConversionContext, LinkResolutionContext,
        convert_inline_node as convert_inline_node_with_context,
        convert_inline_nodes as convert_inline_nodes_with_context, extract_plain_text,
    };
    use crate::convert_ast::blocks::{BlockConversionContext, convert_block_content};

    fn convert_inline_node(node: &RdNode) -> Option<Node> {
        convert_inline_node_with_context(node, &InlineConversionContext::default())
    }

    fn convert_inline_nodes(nodes: &[RdNode]) -> Vec<Node> {
        convert_inline_nodes_with_context(nodes, &InlineConversionContext::default())
    }

    fn text(value: &str) -> RdNode {
        RdNode::Text(value.to_owned())
    }

    fn verb(value: &str) -> RdNode {
        RdNode::Verb(value.to_owned())
    }

    fn tagged(tag: RdTag, children: Vec<RdNode>) -> RdNode {
        RdNode::tagged(tag, None, children)
    }

    fn tagged_with_option(tag: RdTag, option: &str, children: Vec<RdNode>) -> RdNode {
        RdNode::tagged(tag, Some(vec![text(option)]), children)
    }

    fn span(tag: RdTag, value: &str) -> RdNode {
        tagged(tag, vec![text(value)])
    }

    fn group(nodes: Vec<RdNode>) -> RdNode {
        RdNode::group(nodes)
    }

    fn raw(nodes: Vec<RdNode>) -> RdNode {
        RdNode::Raw(producer::raw_node(None, None, nodes, None, vec![]))
    }

    fn render(nodes: Vec<Node>) -> String {
        mdast_to_qmd(
            &Root::new(nodes),
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: false,
            },
        )
    }

    fn convert_paragraph(nodes: &[RdNode]) -> Vec<Node> {
        convert_block_content(
            nodes,
            &BlockConversionContext {
                inline: InlineConversionContext::default(),
                prefer_ascii_math: false,
                enclosing_heading_depth: 2,
            },
        )
    }

    fn figure(file: &str, second: Option<&str>) -> RdNode {
        let mut children = vec![group(vec![verb(file)])];
        if let Some(second) = second {
            children.push(group(vec![verb(second)]));
        }
        tagged(RdTag::Figure, children)
    }

    fn conditional(
        tag: RdTag,
        format: &str,
        then_branch: Vec<RdNode>,
        else_branch: Option<Vec<RdNode>>,
    ) -> RdNode {
        let mut children = vec![group(vec![text(format)]), group(then_branch)];
        if let Some(else_branch) = else_branch {
            children.push(group(else_branch));
        }
        tagged(tag, children)
    }

    fn context_with_html(include_html_output: bool) -> InlineConversionContext<'static> {
        InlineConversionContext {
            include_html_output,
            ..InlineConversionContext::default()
        }
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
    fn normalizes_multiline_text_inside_strong() {
        let node = tagged(RdTag::Strong, vec![text("first\n\nsecond")]);

        assert_eq!(
            convert_inline_node(&node),
            Some(Node::strong(vec![Node::text("first second")]))
        );
    }

    #[test]
    fn preserves_nested_dots_in_code() {
        let node = tagged(
            RdTag::Code,
            vec![text("f("), tagged(RdTag::Dots, vec![]), text(")")],
        );

        assert_eq!(
            convert_inline_node(&node),
            Some(Node::inline_code("f(...)"))
        );
    }

    #[test]
    fn preserves_recovered_text_nested_in_code_raw_node() {
        let node = tagged(RdTag::Code, vec![raw(vec![text("recovered")])]);

        assert_eq!(
            convert_inline_node(&node),
            Some(Node::inline_code("recovered"))
        );
    }

    #[test]
    fn parsed_unknown_macro_preserves_its_children() {
        let converted = convert_paragraph(&[tagged(
            RdTag::Unknown(r"\madeUpTag".into()),
            vec![text("some text")],
        )]);

        assert_eq!(render(converted), "some text\n");
    }

    #[test]
    fn parsed_sexpr_renders_source_as_inline_code() {
        let converted = convert_paragraph(&[tagged(
            RdTag::Sexpr,
            vec![RdNode::RCode("sum(1:10)".into())],
        )]);

        assert_eq!(render(converted), "`sum(1:10)`\n");
    }

    #[test]
    fn empty_raw_node_contributes_nothing() {
        assert!(convert_inline_nodes(&[raw(vec![])]).is_empty());
    }

    #[test]
    fn preserves_resolved_link_nested_directly_in_code() {
        let node = tagged(
            RdTag::Code,
            vec![tagged_with_option(
                RdTag::Link,
                "=alias",
                vec![text("display")],
            )],
        );
        let alias_map = HashMap::from([("alias".to_owned(), "target".to_owned())]);
        let context = InlineConversionContext {
            links: LinkResolutionContext {
                internal_link_url: Some("{file}.qmd#{topic}"),
                alias_map: Some(&alias_map),
                ..LinkResolutionContext::default()
            },
            include_html_output: false,
            prefer_ascii_math: false,
        };

        assert_eq!(
            convert_inline_node_with_context(&node, &context),
            Some(Node::link(
                "target.qmd#alias",
                vec![Node::inline_code("display")],
            ))
        );
    }

    #[test]
    fn delegates_unresolved_link_nested_directly_in_code_to_link_fallback() {
        let link = tagged_with_option(RdTag::Link, "=alias", vec![text("display")]);
        let node = tagged(RdTag::Code, vec![link.clone()]);

        let bare_link_fallback = convert_inline_node(&link);
        assert_eq!(bare_link_fallback, Some(Node::inline_code("display")));
        assert_eq!(convert_inline_node(&node), bare_link_fallback);
    }

    #[test]
    fn flattens_code_with_link_and_additional_child() {
        let node = tagged(
            RdTag::Code,
            vec![
                tagged_with_option(RdTag::Link, "=alias", vec![text("display")]),
                text(" suffix"),
            ],
        );
        let alias_map = HashMap::from([("alias".to_owned(), "target".to_owned())]);
        let context = InlineConversionContext {
            links: LinkResolutionContext {
                internal_link_url: Some("{file}.qmd#{topic}"),
                alias_map: Some(&alias_map),
                ..LinkResolutionContext::default()
            },
            include_html_output: false,
            prefer_ascii_math: false,
        };

        assert_eq!(
            convert_inline_node_with_context(&node, &context),
            Some(Node::inline_code("display suffix"))
        );
    }

    #[test]
    fn preserves_nested_r_symbol_in_single_quotes() {
        let node = tagged(
            RdTag::SQuote,
            vec![text("using "), tagged(RdTag::R, vec![])],
        );

        assert_eq!(convert_inline_node(&node), Some(Node::text("'using R'")));
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
    fn converts_eqn_and_deqn_to_inline_math_via_the_inline_path() {
        // \deqn is a display equation, but convert_inline_node can only ever
        // produce a genuinely inline result (its output may be nested inside
        // Paragraph/Emphasis/Strong/Link children, where a block node is
        // structurally unsafe) -- so it degrades to InlineMath here too.
        // blocks.rs's standalone Deqn handling is what produces true block
        // math.
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
            vec![Node::inline_math("x^2"), Node::inline_math("x = y")]
        );
    }

    #[test]
    fn deqn_via_conditional_stays_structurally_safe_next_to_prose() {
        // A block-display \deqn reached through a conditional branch (the
        // inline path) must not produce a block node (Node::Math/Node::Code)
        // sitting next to plain text inside the same Paragraph -- that would
        // be structurally unsafe for the real writer. Render through the
        // actual writer, not a string-flattening shortcut, and confirm nothing
        // resembling a fenced/attached block leaks into a single paragraph.
        let branch = tagged(
            RdTag::IfElse,
            vec![
                group(vec![text("text")]),
                group(vec![
                    text("before "),
                    tagged(
                        RdTag::Deqn,
                        vec![group(vec![text("x = y")]), group(vec![text("x = y")])],
                    ),
                    text(" after"),
                ]),
                group(vec![text("else")]),
            ],
        );
        let paragraph = Node::paragraph(convert_inline_nodes(&[branch]));

        let markdown = render(vec![paragraph]);
        assert!(
            !markdown.contains("```"),
            "a Deqn reached via the inline path must not render as a fenced block: {markdown:?}"
        );
        assert!(markdown.contains("before"));
        assert!(markdown.contains("after"));
    }

    #[test]
    fn inline_equation_uses_ascii_only_when_preferred_and_non_blank() {
        let with_ascii = tagged(
            RdTag::Eqn,
            vec![group(vec![text("x^2")]), group(vec![text("x squared")])],
        );
        let blank_ascii = tagged(
            RdTag::Eqn,
            vec![group(vec![text("y^2")]), group(vec![text("  \n")])],
        );
        let ascii_context = InlineConversionContext {
            prefer_ascii_math: true,
            ..InlineConversionContext::default()
        };

        assert_eq!(
            convert_inline_node_with_context(&with_ascii, &ascii_context),
            Some(Node::inline_code("x squared")),
        );
        assert_eq!(
            convert_inline_node(&with_ascii),
            Some(Node::inline_math("x^2")),
            "prefer_ascii_math defaults to false"
        );
        assert_eq!(
            convert_inline_node_with_context(&blank_ascii, &ascii_context),
            Some(Node::inline_math("y^2")),
            "blank ascii text falls back to latex"
        );
    }

    #[test]
    fn inline_equation_collapses_multiline_ascii_for_real_writer() {
        let equation = tagged(
            RdTag::Eqn,
            vec![
                group(vec![text("x^2 + y^2")]),
                group(vec![text("x^2 +\n y^2")]),
            ],
        );
        let context = InlineConversionContext {
            prefer_ascii_math: true,
            ..InlineConversionContext::default()
        };
        let node = convert_inline_node_with_context(&equation, &context).unwrap();
        let Node::InlineCode(code) = &node else {
            panic!("expected inline code")
        };

        assert!(!code.value.contains('\n'));
        assert_eq!(render(vec![Node::paragraph(vec![node])]), "`x^2 +  y^2`\n");
    }

    #[test]
    fn inline_equation_collapses_multiline_verb_latex_for_real_writer() {
        let equation = tagged(
            RdTag::Eqn,
            vec![
                group(vec![verb("x^2 +\n y^2")]),
                group(vec![text("x squared plus y squared")]),
            ],
        );
        let node = convert_inline_node_with_context(&equation, &InlineConversionContext::default())
            .unwrap();
        let Node::InlineMath(math) = &node else {
            panic!("expected inline math")
        };

        assert!(!math.value.contains('\n'));
        assert_eq!(render(vec![Node::paragraph(vec![node])]), "$x^2 +  y^2$\n");
    }

    #[test]
    fn skips_malformed_equations() {
        let malformed = tagged(RdTag::Eqn, vec![text("not grouped")]);
        assert_eq!(convert_inline_node(&malformed), None);
    }

    #[test]
    fn plain_if_obeys_format_and_html_output_option() {
        let html = conditional(RdTag::If, "html", vec![text("HTML")], None);
        let text_format = conditional(RdTag::If, "text", vec![text("TEXT")], None);
        let other = conditional(RdTag::If, "latex", vec![text("OTHER")], None);

        assert_eq!(
            convert_inline_node_with_context(&html, &context_with_html(false)),
            None
        );
        assert_eq!(
            convert_inline_node_with_context(&html, &context_with_html(true)),
            Some(Node::text("HTML"))
        );
        for include_html_output in [false, true] {
            let context = context_with_html(include_html_output);
            assert_eq!(
                convert_inline_node_with_context(&text_format, &context),
                Some(Node::text("TEXT"))
            );
            assert_eq!(convert_inline_node_with_context(&other, &context), None);
        }
    }

    #[test]
    fn ifelse_selects_by_format_independently_of_html_output_option() {
        let html = conditional(
            RdTag::IfElse,
            "html",
            vec![text("THEN")],
            Some(vec![text("ELSE")]),
        );
        let text_format = conditional(
            RdTag::IfElse,
            "text",
            vec![text("THEN")],
            Some(vec![text("ELSE")]),
        );
        let other = conditional(
            RdTag::IfElse,
            "latex",
            vec![text("THEN")],
            Some(vec![text("ELSE")]),
        );

        for include_html_output in [false, true] {
            let context = context_with_html(include_html_output);
            assert_eq!(
                convert_inline_node_with_context(&html, &context),
                Some(Node::text("THEN"))
            );
            assert_eq!(
                convert_inline_node_with_context(&text_format, &context),
                Some(Node::text("THEN"))
            );
            assert_eq!(
                convert_inline_node_with_context(&other, &context),
                Some(Node::text("ELSE"))
            );
        }
    }

    #[test]
    fn conditional_collapses_one_node_and_wraps_multiple_nodes() {
        let empty = conditional(RdTag::If, "text", vec![], None);
        let one = conditional(RdTag::If, "text", vec![text("one")], None);
        let multiple = conditional(
            RdTag::If,
            "text",
            vec![text("one "), tagged(RdTag::Strong, vec![text("two")])],
            None,
        );

        assert_eq!(convert_inline_node(&empty), Some(Node::paragraph(vec![])));
        assert_eq!(convert_inline_node(&one), Some(Node::text("one")));
        assert_eq!(
            convert_inline_node(&multiple),
            Some(Node::paragraph(vec![
                Node::text("one "),
                Node::strong(vec![Node::text("two")]),
            ]))
        );
    }

    #[test]
    fn malformed_conditional_is_skipped_without_panicking() {
        let malformed = tagged(RdTag::If, vec![group(vec![text("html")])]);
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
        let cran_package = tagged(RdTag::CranPkg, vec![text("stats")]);
        assert_eq!(convert_inline_node(&cran_package), None);
    }

    #[test]
    fn converts_href_with_recursively_converted_display_markup() {
        let href = tagged(
            RdTag::Href,
            vec![
                group(vec![text("https://example.com/reference")]),
                group(vec![
                    text("the "),
                    tagged(RdTag::Strong, vec![text("reference")]),
                ]),
            ],
        );

        assert_eq!(
            convert_inline_node(&href),
            Some(Node::link(
                "https://example.com/reference",
                vec![
                    Node::text("the "),
                    Node::strong(vec![Node::text("reference")]),
                ],
            ))
        );
    }

    #[test]
    fn resolves_bare_link_through_alias_map_or_falls_back_to_code() {
        let link = tagged(RdTag::Link, vec![text("helper")]);
        assert_eq!(
            convert_inline_node(&link),
            Some(Node::inline_code("helper"))
        );

        let alias_map = HashMap::from([("helper".to_owned(), "utils".to_owned())]);
        let context = InlineConversionContext {
            links: LinkResolutionContext {
                internal_link_url: Some("{file}.qmd#{topic}"),
                unqualified_link_url: Some("https://fallback.example/{topic}"),
                alias_map: Some(&alias_map),
                ..LinkResolutionContext::default()
            },
            include_html_output: false,
            prefer_ascii_math: false,
        };
        assert_eq!(
            convert_inline_node_with_context(&link, &context),
            Some(Node::link(
                "utils.qmd#helper",
                vec![Node::inline_code("helper")],
            ))
        );

        let context_without_internal_template = InlineConversionContext {
            links: LinkResolutionContext {
                unqualified_link_url: Some("https://fallback.example/{topic}"),
                alias_map: Some(&alias_map),
                ..LinkResolutionContext::default()
            },
            include_html_output: false,
            prefer_ascii_math: false,
        };
        assert_eq!(
            convert_inline_node_with_context(&link, &context_without_internal_template),
            Some(Node::inline_code("helper"))
        );
    }

    #[test]
    fn resolves_explicit_unqualified_link_or_falls_back_to_code() {
        let link = tagged_with_option(
            RdTag::Link,
            "=helper",
            vec![text("shown "), tagged(RdTag::Emph, vec![text("name")])],
        );
        assert_eq!(
            convert_inline_node(&link),
            Some(Node::inline_code("shown name"))
        );

        let context = InlineConversionContext {
            links: LinkResolutionContext {
                unqualified_link_url: Some("https://example.com/{topic}.html"),
                ..LinkResolutionContext::default()
            },
            include_html_output: false,
            prefer_ascii_math: false,
        };
        assert_eq!(
            convert_inline_node_with_context(&link, &context),
            Some(Node::link(
                "https://example.com/helper.html",
                vec![Node::inline_code("shown name")],
            ))
        );
    }

    #[test]
    fn resolves_explicit_qualified_link_with_package_url_precedence_or_code_fallback() {
        let link = tagged_with_option(
            RdTag::Link,
            "dplyr:mutate",
            vec![tagged(RdTag::Strong, vec![text("transform")])],
        );
        assert_eq!(
            convert_inline_node(&link),
            Some(Node::inline_code("transform"))
        );

        let package_urls = HashMap::from([(
            "dplyr".to_owned(),
            "https://dplyr.example/{topic}".to_owned(),
        )]);
        let context = InlineConversionContext {
            links: LinkResolutionContext {
                external_link_url: Some("x-r-help:{package}/{topic}"),
                package_urls: Some(&package_urls),
                ..LinkResolutionContext::default()
            },
            include_html_output: false,
            prefer_ascii_math: false,
        };
        assert_eq!(
            convert_inline_node_with_context(&link, &context),
            Some(Node::link(
                "https://dplyr.example/mutate",
                vec![Node::inline_code("transform")],
            ))
        );
    }

    #[test]
    fn resolves_display_topic_qualified_link_externally_or_falls_back_to_code() {
        let link = tagged_with_option(RdTag::Link, "dplyr", vec![text("mutate")]);
        assert_eq!(
            convert_inline_node(&link),
            Some(Node::inline_code("dplyr::mutate"))
        );

        let context = InlineConversionContext {
            links: LinkResolutionContext {
                external_link_url: Some("x-r-help:{package}/{topic}"),
                ..LinkResolutionContext::default()
            },
            include_html_output: false,
            prefer_ascii_math: false,
        };
        assert_eq!(
            convert_inline_node_with_context(&link, &context),
            Some(Node::link(
                "x-r-help:dplyr/mutate",
                vec![Node::inline_code("dplyr::mutate")],
            ))
        );
    }

    #[test]
    fn resolves_qualified_and_unqualified_s4_class_links() {
        let qualified =
            tagged_with_option(RdTag::LinkS4Class, "methods", vec![text("envRefClass")]);
        let unqualified = tagged(RdTag::LinkS4Class, vec![text("MyClass")]);
        assert_eq!(
            convert_inline_nodes(&[qualified.clone(), unqualified.clone()]),
            vec![
                Node::inline_code("methods::envRefClass-class"),
                Node::inline_code("MyClass-class"),
            ]
        );

        let context = InlineConversionContext {
            links: LinkResolutionContext {
                unqualified_link_url: Some("https://example.com/{topic}.html"),
                external_link_url: Some("x-r-help:{package}/{topic}"),
                ..LinkResolutionContext::default()
            },
            include_html_output: false,
            prefer_ascii_math: false,
        };
        assert_eq!(
            convert_inline_nodes_with_context(&[qualified, unqualified], &context),
            vec![
                Node::link(
                    "x-r-help:methods/envRefClass-class",
                    vec![Node::inline_code("methods::envRefClass-class")],
                ),
                Node::link(
                    "https://example.com/MyClass-class.html",
                    vec![Node::inline_code("MyClass-class")],
                ),
            ]
        );
    }

    #[test]
    fn converts_only_curated_doi_shape() {
        let doi = tagged(RdTag::Doi, vec![text("10.1000/xyz")]);
        assert_eq!(
            convert_inline_node(&doi),
            Some(Node::link(
                "https://doi.org/10.1000/xyz",
                vec![Node::text("doi:10.1000/xyz")],
            ))
        );

        let malformed_dois = [
            tagged(RdTag::Doi, vec![]),
            tagged(RdTag::Doi, vec![text("one"), text("two")]),
            tagged(RdTag::Doi, vec![verb("10.1000/xyz")]),
            tagged_with_option(RdTag::Doi, "option", vec![text("10.1000/xyz")]),
        ];
        for malformed in malformed_dois {
            assert_eq!(convert_inline_node(&malformed), None);
        }
    }

    fn method(tag: RdTag, generic: &str, qualifier: &str) -> RdNode {
        tagged(
            tag,
            vec![group(vec![text(generic)]), group(vec![text(qualifier)])],
        )
    }

    #[test]
    fn converts_inline_method_macros_outside_usage_to_bare_generic_call() {
        for tag in [RdTag::Method, RdTag::S3Method, RdTag::S4Method] {
            let node = method(tag, "print", "widget");
            assert_eq!(convert_inline_node(&node), Some(Node::text(r"print()")));
        }
    }

    #[test]
    fn malformed_inline_method_macro_is_dropped_not_panicked() {
        let malformed = tagged(RdTag::Method, vec![text("print")]);
        assert_eq!(convert_inline_node(&malformed), None);
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
