//! Rd AST to mdast conversion
//!
//! Converts an Rd document into an mdast tree for Markdown output.

use crate::ast::{RdDocument, RdNode, RdSection, SectionTag, SpecialChar};
use crate::mdast::{
    Align, DefinitionDescription, DefinitionList, DefinitionTerm, Node, Root, Table, TableCell,
    TableRow,
};
use std::collections::HashMap;

/// Options for Rd to mdast conversion
#[derive(Debug, Clone, Default)]
pub struct ConverterOptions {
    /// File extension for internal links (e.g., "qmd", "md", "html")
    /// If None, internal links become inline code instead of hyperlinks
    pub link_extension: Option<String>,
    /// Alias map: maps alias names to Rd file basenames (without extension)
    /// Used to resolve \link{alias} to the correct target file
    pub alias_map: Option<HashMap<String, String>>,
}

/// Convert an Rd document to mdast
pub fn rd_to_mdast(doc: &RdDocument) -> Root {
    rd_to_mdast_with_options(doc, &ConverterOptions::default())
}

/// Convert an Rd document to mdast with options
pub fn rd_to_mdast_with_options(doc: &RdDocument, options: &ConverterOptions) -> Root {
    let mut converter = Converter::new(options.clone());
    converter.convert_document(doc)
}

/// Converter state
struct Converter {
    /// Current heading depth for sections
    section_depth: u8,
    /// Conversion options
    options: ConverterOptions,
}

impl Converter {
    fn new(options: ConverterOptions) -> Self {
        Self {
            section_depth: 1,
            options,
        }
    }

    fn convert_document(&mut self, doc: &RdDocument) -> Root {
        let mut children = Vec::new();

        // Extract title first
        if let Some(title) = doc.get_section(&SectionTag::Title) {
            let title_text = self.extract_text(&title.content);
            children.push(Node::heading(1, vec![Node::text(title_text.trim())]));
        }

        // Process sections in a logical order
        let section_order = [
            SectionTag::Description,
            SectionTag::Usage,
            SectionTag::Arguments,
            SectionTag::Value,
            SectionTag::Details,
            SectionTag::Note,
            SectionTag::SeeAlso,
            SectionTag::Examples,
            SectionTag::References,
            SectionTag::Author,
        ];

        for tag in &section_order {
            if let Some(section) = doc.get_section(tag) {
                children.extend(self.convert_section(section));
            }
        }

        // Handle custom sections
        for section in &doc.sections {
            if let SectionTag::Section(title) = &section.tag {
                children.push(Node::heading(2, vec![Node::text(title.clone())]));
                children.extend(self.convert_content(&section.content));
            }
        }

        Root::new(children)
    }

    fn convert_section(&mut self, section: &RdSection) -> Vec<Node> {
        let mut nodes = Vec::new();

        let heading_text = match &section.tag {
            SectionTag::Description => "Description",
            SectionTag::Usage => "Usage",
            SectionTag::Arguments => "Arguments",
            SectionTag::Value => "Value",
            SectionTag::Details => "Details",
            SectionTag::Note => "Note",
            SectionTag::SeeAlso => "See Also",
            SectionTag::Examples => "Examples",
            SectionTag::References => "References",
            SectionTag::Author => "Author",
            SectionTag::Format => "Format",
            SectionTag::Source => "Source",
            _ => return nodes, // Skip name, alias, etc.
        };

        nodes.push(Node::heading(2, vec![Node::text(heading_text)]));

        // Special handling for specific sections
        match &section.tag {
            SectionTag::Usage => {
                // Usage code block - not executable
                let code = self.extract_text(&section.content);
                nodes.push(Node::code(Some("r".to_string()), code.trim()));
            }
            SectionTag::Examples => {
                // Examples code block - executable in Quarto
                let code = self.extract_text(&section.content);
                nodes.push(Node::code_with_meta(
                    Some("r".to_string()),
                    Some("executable".to_string()),
                    code.trim(),
                ));
            }
            SectionTag::Arguments => {
                nodes.extend(self.convert_arguments(&section.content));
            }
            _ => {
                nodes.extend(self.convert_content(&section.content));
            }
        }

        nodes
    }

    fn convert_arguments(&mut self, content: &[RdNode]) -> Vec<Node> {
        let mut items = Vec::new();

        for node in content {
            if let RdNode::Item { label, content } = node {
                if let Some(label_nodes) = label {
                    let term = self.convert_inline_nodes(label_nodes);
                    let desc = self.convert_content(content);

                    items.push(Node::DefinitionTerm(DefinitionTerm { children: term }));
                    items.push(Node::DefinitionDescription(DefinitionDescription {
                        children: desc,
                    }));
                }
            }
        }

        if items.is_empty() {
            self.convert_content(content)
        } else {
            vec![Node::DefinitionList(DefinitionList { children: items })]
        }
    }

    fn convert_content(&mut self, nodes: &[RdNode]) -> Vec<Node> {
        let mut result = Vec::new();
        let mut current_para: Vec<Node> = Vec::new();

        for node in nodes {
            match node {
                // Block-level nodes flush the current paragraph
                RdNode::Itemize(items) => {
                    self.flush_paragraph(&mut current_para, &mut result);
                    result.push(self.convert_list(items, false));
                }
                RdNode::Enumerate(items) => {
                    self.flush_paragraph(&mut current_para, &mut result);
                    result.push(self.convert_list(items, true));
                }
                RdNode::Describe(items) => {
                    self.flush_paragraph(&mut current_para, &mut result);
                    result.push(self.convert_describe(items));
                }
                RdNode::Tabular { alignment, rows } => {
                    self.flush_paragraph(&mut current_para, &mut result);
                    result.push(self.convert_table(alignment, rows));
                }
                RdNode::Subsection { title, content } => {
                    self.flush_paragraph(&mut current_para, &mut result);
                    self.section_depth += 1;
                    let depth = (self.section_depth + 1).min(6);
                    result.push(Node::heading(depth, self.convert_inline_nodes(title)));
                    result.extend(self.convert_content(content));
                    self.section_depth -= 1;
                }
                RdNode::Section { title, content } => {
                    self.flush_paragraph(&mut current_para, &mut result);
                    self.section_depth += 1;
                    let depth = (self.section_depth + 1).min(6);
                    result.push(Node::heading(depth, self.convert_inline_nodes(title)));
                    result.extend(self.convert_content(content));
                    self.section_depth -= 1;
                }
                RdNode::Preformatted(code) => {
                    self.flush_paragraph(&mut current_para, &mut result);
                    result.push(Node::code(None, code.clone()));
                }
                RdNode::Deqn { latex, ascii: _ } => {
                    self.flush_paragraph(&mut current_para, &mut result);
                    result.push(Node::math(latex.clone()));
                }

                // Inline nodes accumulate in current paragraph
                RdNode::Text(s) => {
                    // Check for paragraph breaks (double newline)
                    let parts: Vec<&str> = s.split("\n\n").collect();
                    for (i, part) in parts.iter().enumerate() {
                        if i > 0 {
                            self.flush_paragraph(&mut current_para, &mut result);
                        }
                        if !part.trim().is_empty() {
                            current_para.push(Node::text(normalize_whitespace(part)));
                        }
                    }
                }
                _ => {
                    if let Some(inline) = self.convert_inline_node(node) {
                        current_para.push(inline);
                    }
                }
            }
        }

        self.flush_paragraph(&mut current_para, &mut result);
        result
    }

    fn flush_paragraph(&self, para: &mut Vec<Node>, result: &mut Vec<Node>) {
        if !para.is_empty() {
            result.push(Node::paragraph(std::mem::take(para)));
        }
    }

    fn convert_inline_nodes(&self, nodes: &[RdNode]) -> Vec<Node> {
        nodes
            .iter()
            .filter_map(|n| self.convert_inline_node(n))
            .collect()
    }

    fn convert_inline_node(&self, node: &RdNode) -> Option<Node> {
        match node {
            RdNode::Text(s) => Some(Node::text(normalize_whitespace(s))),
            RdNode::Code(children) => {
                let text = self.extract_text(children);
                Some(Node::inline_code(text))
            }
            RdNode::Verb(s) => Some(Node::inline_code(s.clone())),
            RdNode::Emph(children) => Some(Node::emphasis(self.convert_inline_nodes(children))),
            RdNode::Strong(children) => Some(Node::strong(self.convert_inline_nodes(children))),
            RdNode::Href { url, text } => {
                Some(Node::link(url.clone(), self.convert_inline_nodes(text)))
            }
            RdNode::Link {
                package,
                topic,
                text,
            } => {
                // Determine display text
                let display_text = if let Some(text_nodes) = text {
                    self.extract_text(text_nodes)
                } else {
                    topic.clone()
                };

                match (package, &self.options.link_extension) {
                    // External package link - always inline code
                    (Some(pkg), _) => {
                        let display = if text.is_some() {
                            display_text
                        } else {
                            format!("{}::{}", pkg, topic)
                        };
                        Some(Node::inline_code(display))
                    }
                    // Internal link with extension configured - create hyperlink
                    (None, Some(ext)) => {
                        // Resolve alias to target file using alias_map
                        let target_file = self
                            .options
                            .alias_map
                            .as_ref()
                            .and_then(|map| map.get(topic))
                            .map(|s| s.as_str())
                            .unwrap_or(topic);
                        let url = format!("{}.{}", target_file, ext);
                        Some(Node::link(url, vec![Node::inline_code(display_text)]))
                    }
                    // Internal link without extension - just inline code
                    (None, None) => Some(Node::inline_code(display_text)),
                }
            }
            RdNode::Url(url) => Some(Node::link(url.clone(), vec![Node::text(url.clone())])),
            RdNode::Email(email) => {
                let mailto = format!("mailto:{}", email);
                Some(Node::link(mailto, vec![Node::text(email.clone())]))
            }
            RdNode::Pkg(name) => Some(Node::strong(vec![Node::text(name.clone())])),
            RdNode::File(children) => {
                let text = self.extract_text(children);
                Some(Node::inline_code(text))
            }
            RdNode::Var(name) => Some(Node::emphasis(vec![Node::text(name.clone())])),
            RdNode::Eqn { latex, ascii: _ } => Some(Node::inline_math(latex.clone())),
            RdNode::Special(ch) => Some(Node::text(special_char_to_string(*ch))),
            RdNode::LineBreak => Some(Node::Break),
            RdNode::Samp(children) => {
                let text = self.extract_text(children);
                Some(Node::inline_code(text))
            }
            RdNode::SQuote(children) => {
                let text = self.extract_text(children);
                Some(Node::text(format!("'{}'", text)))
            }
            RdNode::DQuote(children) => {
                let text = self.extract_text(children);
                Some(Node::text(format!("\"{}\"", text)))
            }
            RdNode::Acronym(s) => Some(Node::text(s.clone())),
            RdNode::Dfn(children) => Some(Node::emphasis(self.convert_inline_nodes(children))),
            RdNode::Option(s) => Some(Node::inline_code(s.clone())),
            RdNode::Command(s) => Some(Node::inline_code(s.clone())),
            RdNode::Env(s) => Some(Node::inline_code(s.clone())),
            RdNode::Kbd(children) => {
                let text = self.extract_text(children);
                Some(Node::inline_code(text))
            }
            RdNode::If { format, content } => {
                // For markdown/html output, include content if format matches
                if format == "html" || format == "text" {
                    let inline = self.convert_inline_nodes(content);
                    if inline.len() == 1 {
                        inline.into_iter().next()
                    } else {
                        Some(Node::paragraph(inline))
                    }
                } else {
                    None
                }
            }
            RdNode::IfElse {
                format,
                then_content,
                else_content,
            } => {
                let content = if format == "html" || format == "text" {
                    then_content
                } else {
                    else_content
                };
                let inline = self.convert_inline_nodes(content);
                if inline.len() == 1 {
                    inline.into_iter().next()
                } else {
                    Some(Node::paragraph(inline))
                }
            }
            RdNode::Out(html) => Some(Node::Html(crate::mdast::Html {
                value: html.clone(),
            })),
            RdNode::Figure { file, options: _ } => Some(Node::Image(crate::mdast::Image {
                url: file.clone(),
                title: None,
                alt: file.clone(),
            })),
            RdNode::Method { generic, class: _ } => Some(Node::text(format!("{}()", generic))),
            RdNode::S4Method {
                generic,
                signature: _,
            } => Some(Node::text(format!("{}()", generic))),
            // Block nodes handled elsewhere
            _ => None,
        }
    }

    fn convert_list(&self, items: &[RdNode], ordered: bool) -> Node {
        let list_items: Vec<Node> = items
            .iter()
            .filter_map(|item| {
                if let RdNode::Item { content, .. } = item {
                    let children = self.convert_inline_nodes(content);
                    Some(Node::list_item(if children.is_empty() {
                        vec![]
                    } else {
                        vec![Node::paragraph(children)]
                    }))
                } else {
                    None
                }
            })
            .collect();

        Node::list(ordered, list_items)
    }

    fn convert_describe(&self, items: &[crate::ast::DescribeItem]) -> Node {
        let mut children = Vec::new();

        for item in items {
            let term = self.convert_inline_nodes(&item.term);
            let desc = self.convert_inline_nodes(&item.description);

            children.push(Node::DefinitionTerm(DefinitionTerm { children: term }));
            children.push(Node::DefinitionDescription(DefinitionDescription {
                children: if desc.is_empty() {
                    vec![]
                } else {
                    vec![Node::paragraph(desc)]
                },
            }));
        }

        Node::DefinitionList(DefinitionList { children })
    }

    fn convert_table(&self, alignment: &str, rows: &[Vec<Vec<RdNode>>]) -> Node {
        let align: Vec<Option<Align>> = alignment
            .chars()
            .map(|c| match c {
                'l' => Some(Align::Left),
                'c' => Some(Align::Center),
                'r' => Some(Align::Right),
                _ => None,
            })
            .collect();

        let table_rows: Vec<Node> = rows
            .iter()
            .map(|row| {
                let cells: Vec<Node> = row
                    .iter()
                    .map(|cell| {
                        let children = self.convert_inline_nodes(cell);
                        Node::TableCell(TableCell { children })
                    })
                    .collect();
                Node::TableRow(TableRow { children: cells })
            })
            .collect();

        Node::Table(Table {
            align,
            children: table_rows,
        })
    }

    fn extract_text(&self, nodes: &[RdNode]) -> String {
        let mut result = String::new();
        for node in nodes {
            match node {
                RdNode::Text(s) => result.push_str(s),
                RdNode::Code(children) | RdNode::Emph(children) | RdNode::Strong(children) => {
                    result.push_str(&self.extract_text(children));
                }
                RdNode::Link {
                    package,
                    topic,
                    text,
                } => {
                    // Extract display text from link
                    if let Some(text_nodes) = text {
                        result.push_str(&self.extract_text(text_nodes));
                    } else if let Some(pkg) = package {
                        result.push_str(&format!("{}::{}", pkg, topic));
                    } else {
                        result.push_str(topic);
                    }
                }
                RdNode::Method { generic, class: _ } => {
                    // S3 method: use generic function name
                    result.push_str(generic);
                }
                RdNode::S4Method {
                    generic,
                    signature: _,
                } => {
                    // S4 method: use generic function name
                    result.push_str(generic);
                }
                RdNode::Special(ch) => result.push_str(special_char_to_string(*ch)),
                RdNode::LineBreak => result.push('\n'),
                _ => {}
            }
        }
        result
    }
}

fn special_char_to_string(ch: SpecialChar) -> &'static str {
    match ch {
        SpecialChar::R => "R",
        SpecialChar::Dots => "...",
        SpecialChar::LeftBrace => "{",
        SpecialChar::RightBrace => "}",
        SpecialChar::Backslash => "\\",
        SpecialChar::Percent => "%",
        SpecialChar::EnDash => "\u{2013}",
        SpecialChar::EmDash => "\u{2014}",
        SpecialChar::Lsqb => "\u{2018}",
        SpecialChar::Rsqb => "\u{2019}",
        SpecialChar::Ldqb => "\u{201C}",
        SpecialChar::Rdqb => "\u{201D}",
    }
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

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
    fn test_list_conversion() {
        let doc = parse("\\title{T}\n\\details{\\itemize{\\item A\\item B}}").unwrap();
        let mdast = rd_to_mdast(&doc);

        assert!(mdast.children.iter().any(|n| matches!(n, Node::List(_))));
    }

    #[test]
    fn test_internal_link_with_extension() {
        let doc = parse("\\title{T}\n\\description{See \\link{other_func}}").unwrap();
        let options = ConverterOptions {
            link_extension: Some("qmd".to_string()),
            alias_map: None,
        };
        let mdast = rd_to_mdast_with_options(&doc, &options);

        // Find the paragraph with a link
        let has_link = mdast.children.iter().any(|n| {
            if let Node::Paragraph(p) = n {
                p.children.iter().any(|c| {
                    if let Node::Link(l) = c {
                        l.url == "other_func.qmd"
                    } else {
                        false
                    }
                })
            } else {
                false
            }
        });
        assert!(has_link, "Expected internal link to be converted to hyperlink");
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
    fn test_external_link_always_inline_code() {
        let doc = parse("\\title{T}\n\\description{See \\link[dplyr]{filter}}").unwrap();
        let options = ConverterOptions {
            link_extension: Some("qmd".to_string()),
            alias_map: None,
        };
        let mdast = rd_to_mdast_with_options(&doc, &options);

        // External links should always be inline code
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
        assert!(has_inline_code, "Expected external link to be inline code");
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
}
