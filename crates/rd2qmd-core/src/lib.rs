//! Core conversion from the canonical `rd-ast` representation to Quarto Markdown.

pub mod ast_io;
mod convert_ast;
mod options;
mod source_parse;

pub use ast_io::{AST_FORMAT_VERSION, AstIoError, RdAstEnvelope};
pub use options::{ArgumentsFormat, RdToMdastOptions};
pub use rd_ast::{RdDocument, RdNode};
pub use rd2qmd_mdast::{Frontmatter, RdMetadata, WriterOptions, mdast_to_qmd};

/// Frontmatter output options.
#[derive(Debug, Clone, Default)]
pub struct FrontmatterOptions {
    pub enabled: bool,
    pub pagetitle: bool,
}

/// Code block execution options.
#[derive(Debug, Clone)]
pub struct CodeExecutionOptions {
    pub quarto_code_blocks: bool,
    pub exec_dontrun: bool,
    pub exec_donttest: bool,
}

impl Default for CodeExecutionOptions {
    fn default() -> Self {
        Self {
            quarto_code_blocks: true,
            exec_dontrun: false,
            exec_donttest: true,
        }
    }
}

/// Link resolution options.
#[derive(Debug, Clone, Default)]
pub struct LinkOptions {
    pub internal_link_url: Option<String>,
    pub unqualified_link_url: Option<String>,
    pub external_link_url: Option<String>,
    pub alias_map: Option<std::collections::HashMap<String, String>>,
    pub package_urls: Option<std::collections::HashMap<String, String>>,
}

/// Options for single-document conversion.
#[derive(Debug, Clone, Default)]
pub struct RdConvertOptions {
    pub frontmatter: FrontmatterOptions,
    pub code: CodeExecutionOptions,
    pub links: LinkOptions,
    pub arguments_format: ArgumentsFormat,
    pub include_html_output: bool,
    pub prefer_ascii_math: bool,
}

/// Extract plain text while preserving the legacy public helper's fallback spellings.
pub fn extract_text(nodes: &[RdNode]) -> String {
    fn visit(nodes: &[RdNode], out: &mut String) {
        let path = rd_ast::RdPath::new(Vec::new());
        for node in nodes {
            match node {
                RdNode::Text(value) | RdNode::RCode(value) | RdNode::Verb(value) => {
                    out.push_str(value)
                }
                RdNode::Comment(_) => {}
                RdNode::Group(group) => visit(group.children(), out),
                RdNode::Raw(raw) => visit(raw.children(), out),
                RdNode::Tagged(tagged) => {
                    if tagged.tag() == &rd_ast::RdTag::Link {
                        if let Ok(link) = tagged.inspect_link(&path) {
                            // Mirrors convert_ast::inline::convert_link's display
                            // logic: `\link[pkg]{topic}` (no explicit override)
                            // shows "pkg::topic", but every other form (bare
                            // `\link{topic}`, `\link[=dest]{label}`,
                            // `\link[pkg:topic]{label}`) shows the tag's
                            // children (`display()`) verbatim.
                            match link.destination() {
                                rd_ast::RdLinkDestination::Package {
                                    package,
                                    topic: rd_ast::RdLinkTopic::DisplayText(nodes),
                                } => {
                                    out.push_str(package);
                                    out.push_str("::");
                                    visit(nodes, out);
                                }
                                _ => visit(link.display(), out),
                            }
                        } else {
                            visit(tagged.children(), out);
                        }
                    } else if tagged.tag() == &rd_ast::RdTag::Href {
                        if let Ok(href) = tagged.inspect_href(&path) {
                            visit(href.display(), out)
                        } else {
                            visit(tagged.children(), out);
                        }
                    } else if tagged.tag() == &rd_ast::RdTag::LinkS4Class {
                        if let Some(link) = node.s4_class_link(&path)
                            && let Some(class) = link.class_text()
                        {
                            if let Some(package) = link.package_text() {
                                out.push_str(&package);
                                out.push_str("::");
                            }
                            out.push_str(&class);
                        }
                    } else if tagged.tag() == &rd_ast::RdTag::Doi {
                        if let [RdNode::Text(id)] = tagged.children() {
                            out.push_str("doi:");
                            out.push_str(id);
                        } else {
                            visit(tagged.children(), out);
                        }
                    } else if tagged.tag() == &rd_ast::RdTag::Enc {
                        if let Some(enc) = node.enc(&path) {
                            visit(enc.encoded(), out);
                        }
                    } else if let Some(span) = node.inline_span(&path) {
                        visit(span.body(), out);
                    } else if let Some(symbol) = node.text_symbol(&path) {
                        out.push_str(symbol.fallback_text());
                    } else {
                        visit(tagged.children(), out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut result = String::new();
    visit(nodes, &mut result);
    result.trim().to_owned()
}

/// Extract metadata directly from the canonical AST.
pub fn extract_rd_metadata(doc: &RdDocument) -> RdMetadata {
    let metadata = convert_ast::extract_document_metadata(doc);
    RdMetadata {
        lifecycle: metadata.lifecycle,
        aliases: metadata.aliases,
        keywords: metadata.keywords,
        concepts: metadata.concepts,
        source_files: metadata.source_files,
    }
}

/// Convert an already-parsed canonical Rd document to Quarto Markdown.
pub fn convert_rd_document(doc: &RdDocument, options: &RdConvertOptions) -> String {
    let converter_options = RdToMdastOptions {
        include_title_heading: !options.frontmatter.enabled,
        internal_link_url: options.links.internal_link_url.clone(),
        alias_map: options.links.alias_map.clone(),
        unqualified_link_url: options.links.unqualified_link_url.clone(),
        package_urls: options.links.package_urls.clone(),
        external_link_url: options.links.external_link_url.clone(),
        exec_dontrun: options.code.exec_dontrun,
        exec_donttest: options.code.exec_donttest,
        quarto_code_blocks: options.code.quarto_code_blocks,
        arguments_format: options.arguments_format.clone(),
        include_html_output: options.include_html_output,
        prefer_ascii_math: options.prefer_ascii_math,
    };
    let mdast = convert_ast::convert_document(doc, &converter_options);
    let title = doc.title().map(extract_text);
    let name = doc.name().map(extract_text);
    let pagetitle = options
        .frontmatter
        .pagetitle
        .then(|| match (&title, &name) {
            (Some(title), Some(name)) => format!("{title} — {name}"),
            _ => String::new(),
        })
        .filter(|value| !value.is_empty());
    let writer_options = WriterOptions {
        frontmatter: options.frontmatter.enabled.then_some(Frontmatter {
            title,
            pagetitle,
            format: None,
            metadata: Some(extract_rd_metadata(doc)),
        }),
        quarto_code_blocks: options.code.quarto_code_blocks,
    };
    mdast_to_qmd(&mdast, &writer_options)
}

/// Convert a document to mdast with default options.
pub fn rd_to_mdast(doc: &RdDocument) -> rd2qmd_mdast::Root {
    rd_to_mdast_with_options(doc, &RdToMdastOptions::default())
}

/// Convert a document to mdast with custom options.
pub fn rd_to_mdast_with_options(
    doc: &RdDocument,
    options: &RdToMdastOptions,
) -> rd2qmd_mdast::Root {
    convert_ast::convert_document(doc, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> RdDocument {
        rd_source::parse(source.as_bytes())
            .unwrap()
            .document()
            .clone()
    }

    #[test]
    fn extract_text_preserves_legacy_inline_fallbacks() {
        let doc = parse(
            r#"\description{Use \code{foo()} and \emph{bar}. \link[pkg]{topic} \link{plain} \linkS4class[pkg]{Class} \doi{10.1000/xyz}.}"#,
        );
        let description = doc.description().unwrap();
        assert_eq!(
            extract_text(description),
            "Use foo() and bar. pkg::topic plain pkg::Class doi:10.1000/xyz."
        );
    }

    #[test]
    fn extract_text_uses_display_label_for_explicit_link_destinations() {
        let doc = parse(
            r#"\description{See \link[=dest]{explicit label} and \link[pkg:topic]{qualified label}.}"#,
        );
        assert_eq!(
            extract_text(doc.description().unwrap()),
            "See explicit label and qualified label."
        );
    }

    #[test]
    fn extract_text_uses_href_display_not_url() {
        let doc = parse(r#"\description{Visit \href{https://example.com}{the site}.}"#);
        assert_eq!(extract_text(doc.description().unwrap()), "Visit the site.");
    }

    #[test]
    fn extract_text_handles_encoded_fallback_and_special_r() {
        let doc = parse(r#"\description{Using \enc{café}{latin1} and \R.}"#);
        assert_eq!(
            extract_text(doc.description().unwrap()),
            "Using café and R."
        );
    }
}
