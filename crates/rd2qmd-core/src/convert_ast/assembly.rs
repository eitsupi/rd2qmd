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
    use rd2qmd_mdast::{WriterOptions, mdast_to_qmd};

    use super::convert_document;
    use crate::RdToMdastOptions;

    fn parse_ast(source: &str) -> rd_ast::RdDocument {
        let parsed = rd_source::parse(source.as_bytes()).unwrap();
        assert!(
            parsed.diagnostics().is_empty(),
            "unexpected parse diagnostics: {:?}",
            parsed.diagnostics()
        );
        parsed.document().clone()
    }

    fn render_new(source: &str, options: &RdToMdastOptions) -> String {
        let root = convert_document(&parse_ast(source), options);
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
        let source = r#"
\name{topic}
\title{Topic Title}
\examples{topic(1)}
\author{Author body.}
\seealso{See also body.}
\references{References body.}
\note{Note body.}
\source{Source body.}
\format{Format body.}
\details{Details body.}
\value{Value body.}
\arguments{\item{x}{Argument body.}}
\usage{topic(x)}
\description{Description body.}
"#;
        let options = RdToMdastOptions::default();
        let markdown = render_new(source, &options);

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
        let source = r#"
\name{topic}
\title{Topic Title}
\section{First Custom}{First custom body.}
\examples{topic()}
\details{Details body.}
\section{Second Custom}{Second custom body.}
\description{Description body.}
"#;
        let options = RdToMdastOptions::default();
        let markdown = render_new(source, &options);

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
        let source = r#"\name{topic}\title{Topic Title}\description{Body.}"#;
        let with_title = RdToMdastOptions::default();
        let without_title = RdToMdastOptions {
            include_title_heading: false,
            ..RdToMdastOptions::default()
        };

        let with_title_markdown = render_new(source, &with_title);
        let without_title_markdown = render_new(source, &without_title);
        assert_eq!(headings(&with_title_markdown)[0], "# Topic Title");
        assert_eq!(headings(&without_title_markdown), ["## Description"]);
    }

    #[test]
    fn conditional_rendering_matches_legacy_for_html_output_option() {
        let source = r#"
\name{topic}
\title{Conditional rendering}
\description{\if{html}{html-only}\if{text}{text-always}\ifelse{html}{html-then}{html-else}}
"#;

        for include_html_output in [false, true] {
            let options = RdToMdastOptions {
                include_html_output,
                ..RdToMdastOptions::default()
            };
            let markdown = render_new(source, &options);

            assert_eq!(markdown.contains("html-only"), include_html_output);
            assert!(markdown.contains("text-always"));
            assert!(markdown.contains("html-then"));
            assert!(!markdown.contains("html-else"));
        }
    }
}
