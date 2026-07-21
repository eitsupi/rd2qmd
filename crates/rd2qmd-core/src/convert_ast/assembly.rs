//! Whole-document assembly for the rd_ast conversion path.

use rd2qmd_mdast::{Node, Root};

use super::{
    BlockConversionContext, DocumentSection, ExampleOptions, FixedSection, FixedSectionBody,
    FixedSectionKind, InlineConversionContext, LinkResolutionContext, build_document_structure,
    convert_arguments, convert_block_content, convert_custom_section, convert_examples,
    convert_usage,
};

/// Convert one rd-ast document into a complete mdast root.
pub(crate) fn convert_document(
    doc: &rd_ast::RdDocument,
    options: &crate::RdToMdastOptions,
) -> Root {
    let structure = build_document_structure(doc);
    let context = block_context(options);
    let mut children = Vec::new();

    if options.include_title_heading
        && let Some(title) = structure.title
    {
        children.push(Node::heading(1, vec![Node::text(title)]));
    }

    for section in &structure.sections {
        match section {
            DocumentSection::Fixed(section) => {
                children.extend(convert_fixed_section(section, options, &context));
            }
            DocumentSection::Custom(section) => {
                children.extend(convert_custom_section(section, &context));
            }
        }
    }

    Root::new(children)
}

fn block_context<'a>(options: &'a crate::RdToMdastOptions) -> BlockConversionContext<'a> {
    BlockConversionContext {
        inline: InlineConversionContext {
            links: LinkResolutionContext {
                internal_link_url: options.internal_link_url.as_deref(),
                unqualified_link_url: options.unqualified_link_url.as_deref(),
                external_link_url: options.external_link_url.as_deref(),
                alias_map: options.alias_map.as_ref(),
                package_urls: options.package_urls.as_ref(),
            },
            include_html_output: options.include_html_output,
            prefer_ascii_math: options.prefer_ascii_math,
        },
        prefer_ascii_math: options.prefer_ascii_math,
        enclosing_heading_depth: 2,
    }
}

fn convert_fixed_section(
    section: &FixedSection<'_>,
    options: &crate::RdToMdastOptions,
    context: &BlockConversionContext<'_>,
) -> Vec<Node> {
    let mut nodes = vec![Node::heading(2, vec![Node::text(section.kind.heading())])];

    match &section.body {
        FixedSectionBody::Arguments(arguments) => nodes.extend(convert_arguments(
            arguments,
            options.arguments_format.clone(),
            context,
        )),
        FixedSectionBody::Nodes(body) => match section.kind {
            FixedSectionKind::Usage => {
                nodes.push(Node::code(Some("r".to_owned()), convert_usage(body).trim()));
            }
            FixedSectionKind::Examples => nodes.extend(convert_examples(
                body,
                &ExampleOptions {
                    exec_dontrun: options.exec_dontrun,
                    exec_donttest: options.exec_donttest,
                    quarto_code_blocks: options.quarto_code_blocks,
                },
            )),
            _ => nodes.extend(convert_block_content(body, context)),
        },
    }

    nodes
}

#[cfg(test)]
mod tests {
    use rd_ast::{RdDocument, RdNode, RdTag};
    use rd2qmd_mdast::{WriterOptions, mdast_to_qmd};

    use super::convert_document;
    use crate::RdToMdastOptions;

    fn section(tag: RdTag, text: &str) -> RdNode {
        RdNode::tagged(tag, None, vec![RdNode::Text(text.to_owned())])
    }

    fn custom(title: &str, body: &str) -> RdNode {
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![
                RdNode::group(vec![RdNode::Text(title.to_owned())]),
                RdNode::group(vec![RdNode::Text(body.to_owned())]),
            ],
        )
    }

    fn argument(name: &str, body: &str) -> RdNode {
        RdNode::tagged(
            RdTag::Item,
            None,
            vec![
                RdNode::group(vec![RdNode::Text(name.to_owned())]),
                RdNode::group(vec![RdNode::Text(body.to_owned())]),
            ],
        )
    }

    fn render_new(document: &RdDocument, options: &RdToMdastOptions) -> String {
        let root = convert_document(document, options);
        render(&root, options)
    }

    fn render(root: &rd2qmd_mdast::Root, options: &RdToMdastOptions) -> String {
        mdast_to_qmd(
            root,
            &WriterOptions {
                frontmatter: None,
                quarto_code_blocks: options.quarto_code_blocks,
            },
        )
    }

    fn headings(markdown: &str) -> Vec<&str> {
        markdown
            .lines()
            .filter(|line| line.starts_with('#'))
            .collect()
    }

    #[test]
    fn renders_every_fixed_section_in_legacy_order() {
        let document = RdDocument::new(vec![
            section(RdTag::Name, "topic"),
            section(RdTag::Title, "Topic Title"),
            section(RdTag::Examples, "topic(1)"),
            section(RdTag::Author, "Author body."),
            section(RdTag::SeeAlso, "See also body."),
            section(RdTag::References, "References body."),
            section(RdTag::Note, "Note body."),
            section(RdTag::Source, "Source body."),
            section(RdTag::Format, "Format body."),
            section(RdTag::Details, "Details body."),
            section(RdTag::Value, "Value body."),
            RdNode::tagged(
                RdTag::Arguments,
                None,
                vec![argument("x", "Argument body.")],
            ),
            RdNode::tagged(RdTag::Usage, None, vec![RdNode::RCode("topic(x)".into())]),
            section(RdTag::Description, "Description body."),
        ]);
        let options = RdToMdastOptions::default();
        let markdown = render_new(&document, &options);

        assert_eq!(
            headings(&markdown),
            [
                "# Topic Title",
                "## Description",
                "## Usage",
                "## Arguments",
                "## Value",
                "## Details",
                "## Format",
                "## Source",
                "## Note",
                "## References",
                "## Author",
                "## See Also",
                "## Examples",
            ]
        );
    }

    #[test]
    fn moves_interleaved_custom_sections_after_fixed_sections_and_before_examples() {
        let document = RdDocument::new(vec![
            section(RdTag::Name, "topic"),
            section(RdTag::Title, "Topic Title"),
            custom("First Custom", "First custom body."),
            section(RdTag::Examples, "topic()"),
            section(RdTag::Details, "Details body."),
            custom("Second Custom", "Second custom body."),
            section(RdTag::Description, "Description body."),
        ]);
        let options = RdToMdastOptions::default();
        let markdown = render_new(&document, &options);

        assert_eq!(
            headings(&markdown),
            [
                "# Topic Title",
                "## Description",
                "## Details",
                "## First Custom",
                "## Second Custom",
                "## Examples",
            ]
        );
        assert!(markdown.find("First Custom").unwrap() < markdown.find("Second Custom").unwrap());
    }

    #[test]
    fn title_heading_respects_include_title_heading() {
        let document = RdDocument::new(vec![
            section(RdTag::Name, "topic"),
            section(RdTag::Title, "Topic Title"),
            section(RdTag::Description, "Body."),
        ]);
        let with_title = RdToMdastOptions::default();
        let without_title = RdToMdastOptions {
            include_title_heading: false,
            ..RdToMdastOptions::default()
        };

        let with_title_markdown = render_new(&document, &with_title);
        let without_title_markdown = render_new(&document, &without_title);
        assert_eq!(headings(&with_title_markdown)[0], "# Topic Title");
        assert_eq!(headings(&without_title_markdown), ["## Description"]);
    }

    #[test]
    fn conditional_rendering_matches_legacy_for_html_output_option() {
        let conditional = |tag: RdTag, format: &str, then_text: &str, else_text: Option<&str>| {
            let mut children = vec![
                RdNode::group(vec![RdNode::Text(format.into())]),
                RdNode::group(vec![RdNode::Text(then_text.into())]),
            ];
            if let Some(else_text) = else_text {
                children.push(RdNode::group(vec![RdNode::Text(else_text.into())]));
            }
            RdNode::tagged(tag, None, children)
        };
        let document = RdDocument::new(vec![
            section(RdTag::Name, "topic"),
            section(RdTag::Title, "Conditional rendering"),
            RdNode::tagged(
                RdTag::Description,
                None,
                vec![
                    conditional(RdTag::If, "html", "html-only", None),
                    conditional(RdTag::If, "text", "text-always", None),
                    conditional(RdTag::IfElse, "html", "html-then", Some("html-else")),
                ],
            ),
        ]);

        for include_html_output in [false, true] {
            let options = RdToMdastOptions {
                include_html_output,
                ..RdToMdastOptions::default()
            };
            let markdown = render_new(&document, &options);

            assert_eq!(markdown.contains("html-only"), include_html_output);
            assert!(markdown.contains("text-always"));
            assert!(markdown.contains("html-then"));
            assert!(!markdown.contains("html-else"));
        }
    }
}
