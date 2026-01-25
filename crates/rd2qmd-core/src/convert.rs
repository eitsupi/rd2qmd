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
#[derive(Debug, Clone)]
pub struct ConverterOptions {
    /// File extension for internal links (e.g., "qmd", "md", "html")
    /// If None, internal links become inline code instead of hyperlinks
    pub link_extension: Option<String>,
    /// Alias map: maps alias names to Rd file basenames (without extension)
    /// Used to resolve \link{alias} to the correct target file
    pub alias_map: Option<HashMap<String, String>>,
    /// URL pattern for unresolved links (fallback to base R documentation)
    /// Use `{topic}` as placeholder for the topic name.
    /// Example: "https://rdrr.io/r/base/{topic}.html"
    /// If None, unresolved links become inline code instead of hyperlinks
    pub unresolved_link_url: Option<String>,
    /// External package URL map: package name -> reference documentation base URL
    /// Used to resolve \link[pkg]{topic} to external package documentation
    /// Example: "dplyr" -> "https://dplyr.tidyverse.org/reference"
    /// The full URL is constructed as "{base_url}/{topic}.html"
    pub external_package_urls: Option<HashMap<String, String>>,
    /// Make \dontrun{} example code executable (default: false, shown as non-executable)
    /// This matches pkgdown's semantics: \dontrun{} means "never run this code"
    pub exec_dontrun: bool,
    /// Make \donttest{} example code executable (default: true, shown as executable)
    /// This matches pkgdown's semantics: \donttest{} means "don't run during testing"
    /// but the code should normally be executable
    pub exec_donttest: bool,
    /// Use Quarto {r} code blocks (affects \dontshow{} handling)
    /// When true, \dontshow{} content is output with #| include: false
    /// When false, \dontshow{} content is skipped entirely
    pub quarto_code_blocks: bool,
}

impl Default for ConverterOptions {
    fn default() -> Self {
        Self {
            link_extension: None,
            alias_map: None,
            unresolved_link_url: None,
            external_package_urls: None,
            exec_dontrun: false,
            exec_donttest: true, // pkgdown-compatible: \donttest{} is executable by default
            quarto_code_blocks: true,
        }
    }
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

        // Process sections in pkgdown order (Examples always last)
        // Standard sections come first in a fixed order
        let section_order = [
            SectionTag::Description,
            SectionTag::Usage,
            SectionTag::Arguments,
            SectionTag::Value,
            SectionTag::Details,
            SectionTag::Format,
            SectionTag::Source,
            SectionTag::Note,
            SectionTag::References,
            SectionTag::Author,
            SectionTag::SeeAlso,
            // Examples handled separately at the end
        ];

        for tag in &section_order {
            if let Some(section) = doc.get_section(tag) {
                children.extend(self.convert_section(section));
            }
        }

        // Handle custom sections (before Examples, in original Rd order)
        for section in &doc.sections {
            if let SectionTag::Section(title) = &section.tag {
                children.push(Node::heading(2, vec![Node::text(title.clone())]));
                children.extend(self.convert_content(&section.content));
            }
        }

        // Examples always last (pkgdown convention)
        if let Some(section) = doc.get_section(&SectionTag::Examples) {
            children.extend(self.convert_section(section));
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
                // Examples section - may contain regular code, \dontrun{}, and \donttest{}
                nodes.extend(self.convert_examples(&section.content));
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

    /// Convert examples section content, handling \dontrun{} and \donttest{} based on mode
    fn convert_examples(&self, content: &[RdNode]) -> Vec<Node> {
        let mut result = Vec::new();
        let mut current_code = String::new();
        let mut has_executable = false;

        // Helper to flush accumulated code as a code block
        let flush_code = |code: &mut String, result: &mut Vec<Node>, executable: bool| {
            let trimmed = code.trim();
            if !trimmed.is_empty() {
                if executable {
                    result.push(Node::code_with_meta(
                        Some("r".to_string()),
                        Some("executable".to_string()),
                        trimmed,
                    ));
                } else {
                    result.push(Node::code(Some("r".to_string()), trimmed));
                }
            }
            code.clear();
        };

        for node in content {
            match node {
                RdNode::DontRun(children) => {
                    // Flush any accumulated regular code first
                    flush_code(&mut current_code, &mut result, true);
                    has_executable = false;

                    let code = self.extract_text(children);
                    let trimmed = code.trim();
                    if !trimmed.is_empty() {
                        if self.options.exec_dontrun {
                            result.push(Node::code_with_meta(
                                Some("r".to_string()),
                                Some("executable".to_string()),
                                trimmed,
                            ));
                        } else {
                            result.push(Node::code(Some("r".to_string()), trimmed));
                        }
                    }
                }
                RdNode::DontTest(children) => {
                    // Flush any accumulated regular code first
                    flush_code(&mut current_code, &mut result, true);
                    has_executable = false;

                    let code = self.extract_text(children);
                    let trimmed = code.trim();
                    if !trimmed.is_empty() {
                        if self.options.exec_donttest {
                            result.push(Node::code_with_meta(
                                Some("r".to_string()),
                                Some("executable".to_string()),
                                trimmed,
                            ));
                        } else {
                            result.push(Node::code(Some("r".to_string()), trimmed));
                        }
                    }
                }
                RdNode::DontShow(children) => {
                    // \dontshow{} has two usage patterns:
                    // 1. Complete code (setup, etc.) - hide but execute
                    // 2. Wrapper pattern for @examplesIf - skip entirely
                    //
                    // Wrapper pattern detection (structural):
                    // - Start wrapper: has unclosed `{` (more `{` than `}`)
                    // - End wrapper: starts with `}` (closing a brace opened elsewhere)
                    //
                    // NOTE: This brace-counting detection is a simple heuristic.
                    // Future improvement: Consider using tree-sitter for R code
                    // completeness evaluation to more robustly detect wrapper patterns
                    // (e.g., detecting syntactically incomplete expressions).
                    let code = self.extract_text(children);
                    let trimmed = code.trim();

                    // Count braces to detect incomplete code
                    let open_braces = trimmed.chars().filter(|&c| c == '{').count();
                    let close_braces = trimmed.chars().filter(|&c| c == '}').count();

                    // Start wrapper: more `{` than `}` (unclosed brace)
                    let is_start_wrapper = open_braces > close_braces;
                    // End wrapper: starts with `}` (closing something opened elsewhere)
                    let is_end_wrapper = trimmed.starts_with('}');

                    if is_start_wrapper || is_end_wrapper {
                        // Wrapper pattern - skip entirely
                        // The inner code (between start and end wrappers) will be
                        // output as regular executable code
                    } else if !trimmed.is_empty() {
                        // Complete code - hide but execute
                        // Flush any accumulated regular code first
                        flush_code(&mut current_code, &mut result, true);
                        has_executable = false;

                        // For qmd format, use #| include: false to hide but execute
                        // For md format, just skip (no execution anyway)
                        if self.options.quarto_code_blocks {
                            let code_with_directive = format!("#| include: false\n{}", trimmed);
                            result.push(Node::code_with_meta(
                                Some("r".to_string()),
                                Some("executable".to_string()),
                                code_with_directive,
                            ));
                        }
                        // For non-Quarto formats, skip entirely (code won't be executed anyway)
                    }
                }
                _ => {
                    // Regular content - accumulate
                    has_executable = true;
                    current_code.push_str(&self.extract_text(std::slice::from_ref(node)));
                }
            }
        }

        // Flush remaining code
        if has_executable {
            flush_code(&mut current_code, &mut result, true);
        } else if !current_code.trim().is_empty() {
            flush_code(&mut current_code, &mut result, false);
        }

        result
    }

    // TODO: Consider implementing HTML table support for complex argument descriptions
    // containing nested lists. See beads task rd2md-68z for investigation notes.
    // GFM tables cannot contain block elements (lists, multiple paragraphs).
    // Current workaround: use <br> for line breaks, flatten nested lists with bullet markers.
    fn convert_arguments(&mut self, content: &[RdNode]) -> Vec<Node> {
        // Build header row
        let header_row = Node::TableRow(TableRow {
            children: vec![
                Node::TableCell(TableCell {
                    children: vec![Node::text("Argument")],
                }),
                Node::TableCell(TableCell {
                    children: vec![Node::text("Description")],
                }),
            ],
        });

        let mut rows = vec![header_row];

        for node in content {
            if let RdNode::Item { label, content } = node
                && let Some(label_nodes) = label
            {
                // Argument name as inline code
                let term_text = self.extract_text(label_nodes);
                let arg_cell = Node::TableCell(TableCell {
                    children: vec![Node::inline_code(term_text.trim())],
                });

                // Convert description to flat inline content for GFM table cell
                let desc_content = self.flatten_for_table_cell(content);
                let desc_cell = Node::TableCell(TableCell {
                    children: desc_content,
                });

                rows.push(Node::TableRow(TableRow {
                    children: vec![arg_cell, desc_cell],
                }));
            }
        }

        if rows.len() <= 1 {
            // Only header, no data rows - fall back to regular content
            self.convert_content(content)
        } else {
            vec![Node::Table(Table {
                align: vec![Some(Align::Left), Some(Align::Left)],
                children: rows,
            })]
        }
    }

    /// Flatten block content to inline nodes for GFM table cells.
    /// Uses <br> for paragraph breaks and flattens lists with bullet markers.
    fn flatten_for_table_cell(&mut self, content: &[RdNode]) -> Vec<Node> {
        let block_nodes = self.convert_content(content);
        let mut result = Vec::new();

        for (i, node) in block_nodes.iter().enumerate() {
            if i > 0 && !result.is_empty() {
                // Add line break between blocks
                result.push(Node::Html(crate::mdast::Html {
                    value: " <br>".to_string(),
                }));
            }

            match node {
                Node::Paragraph(p) => {
                    result.extend(p.children.clone());
                }
                Node::List(l) => {
                    // Flatten list items with bullet markers
                    for (j, item) in l.children.iter().enumerate() {
                        // Add <br> between list items (not before first item -
                        // block separator already handles gap from previous content)
                        if j > 0 {
                            result.push(Node::Html(crate::mdast::Html {
                                value: " <br>".to_string(),
                            }));
                        }
                        if let Node::ListItem(li) = item {
                            // Add bullet marker
                            let marker = if l.ordered {
                                format!("{}. ", j + 1)
                            } else {
                                "- ".to_string()
                            };
                            result.push(Node::text(marker));
                            // Add item content (first paragraph only for simplicity)
                            for item_child in &li.children {
                                if let Node::Paragraph(p) = item_child {
                                    result.extend(p.children.clone());
                                    break; // Only first paragraph
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Other block elements - skip for table cells
                }
            }
        }

        result
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
                // Check if \code contains a single \link - if so, preserve the link
                // This handles patterns like \code{\link[=alias]{text}}
                if children.len() == 1
                    && let RdNode::Link { .. } = &children[0]
                {
                    // Delegate to link conversion which already wraps text in inline code
                    return self.convert_inline_node(&children[0]);
                }
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
                    // External package link
                    (Some(pkg), _) => {
                        // Extract just the package name (before colon if present)
                        // e.g., "rlang:dyn-dots" -> "rlang"
                        let pkg_name = pkg.split(':').next().unwrap_or(pkg);

                        // Extract the actual topic from the pkg string if it contains ':'
                        // e.g., "rlang:dyn-dots" -> topic is "dyn-dots"
                        let actual_topic = if pkg.contains(':') {
                            pkg.split(':').nth(1).unwrap_or(topic)
                        } else {
                            topic
                        };

                        let display = if text.is_some() {
                            display_text
                        } else {
                            format!("{}::{}", pkg_name, actual_topic)
                        };

                        // Check if we have a URL for this external package
                        if let Some(base_url) = self
                            .options
                            .external_package_urls
                            .as_ref()
                            .and_then(|map| map.get(pkg_name))
                        {
                            let url =
                                format!("{}/{}.html", base_url.trim_end_matches('/'), actual_topic);
                            Some(Node::link(url, vec![Node::inline_code(display)]))
                        } else {
                            // No URL found - just inline code
                            Some(Node::inline_code(display))
                        }
                    }
                    // Internal link with extension configured - create hyperlink
                    (None, Some(ext)) => {
                        // Resolve alias to target file using alias_map
                        if let Some(target_file) = self
                            .options
                            .alias_map
                            .as_ref()
                            .and_then(|map| map.get(topic))
                        {
                            // Found in local package - create relative link
                            let url = format!("{}.{}", target_file, ext);
                            Some(Node::link(url, vec![Node::inline_code(display_text)]))
                        } else if let Some(pattern) = &self.options.unresolved_link_url {
                            // Not found - use fallback URL pattern
                            let url = pattern.replace("{topic}", topic);
                            Some(Node::link(url, vec![Node::inline_code(display_text)]))
                        } else {
                            // No fallback configured - just inline code
                            Some(Node::inline_code(display_text))
                        }
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
            RdNode::Figure { file, options } => {
                let alt = options
                    .as_ref()
                    .and_then(|opts| Self::extract_figure_alt(opts))
                    .unwrap_or_else(|| file.clone());
                Some(Node::Image(crate::mdast::Image {
                    url: file.clone(),
                    title: None,
                    alt,
                }))
            }
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

    /// Extract alt text from figure options string
    /// Handles formats like "options: alt='[Deprecated]'" or "alt='text'"
    fn extract_figure_alt(options: &str) -> Option<String> {
        // Try single quotes: alt='...'
        if let Some(start) = options.find("alt='") {
            let after_quote = &options[start + 5..];
            if let Some(end) = after_quote.find('\'') {
                return Some(after_quote[..end].to_string());
            }
        }
        // Try double quotes: alt="..."
        if let Some(start) = options.find("alt=\"") {
            let after_quote = &options[start + 5..];
            if let Some(end) = after_quote.find('"') {
                return Some(after_quote[..end].to_string());
            }
        }
        None
    }

    fn extract_text(&self, nodes: &[RdNode]) -> String {
        let mut result = String::new();
        let mut i = 0;
        while i < nodes.len() {
            match &nodes[i] {
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
                RdNode::Method { generic, class } => {
                    // S3 method: add comment like pkgdown
                    if class == "default" {
                        result.push_str("# Default S3 method\n");
                    } else {
                        result.push_str(&format!("# S3 method for class '{}'\n", class));
                    }
                    // Check if this is an infix operator and try to format naturally
                    if let Some(formatted) = self.try_format_infix_method(generic, nodes, i + 1) {
                        result.push_str(&formatted.text);
                        i += formatted.nodes_consumed;
                        i += 1;
                        continue;
                    }
                    result.push_str(generic);
                }
                RdNode::S4Method { generic, signature } => {
                    // S4 method: add comment like pkgdown
                    result.push_str(&format!("# S4 method for signature '{}'\n", signature));
                    // Check if this is an infix operator and try to format naturally
                    if let Some(formatted) = self.try_format_infix_method(generic, nodes, i + 1) {
                        result.push_str(&formatted.text);
                        i += formatted.nodes_consumed;
                        i += 1;
                        continue;
                    }
                    result.push_str(generic);
                }
                RdNode::Special(ch) => result.push_str(special_char_to_string(*ch)),
                RdNode::LineBreak => result.push('\n'),
                _ => {}
            }
            i += 1;
        }
        result
    }

    /// Try to format a method as an infix expression (e.g., `e1 + e2` instead of `+(e1, e2)`)
    fn try_format_infix_method(
        &self,
        generic: &str,
        nodes: &[RdNode],
        next_idx: usize,
    ) -> Option<InfixFormatResult> {
        // Check if generic is an infix operator
        if !is_infix_operator(generic) {
            return None;
        }

        // Look for the arguments text starting with '('
        let mut args_text = String::new();
        let mut nodes_consumed = 0;

        for node in nodes.iter().skip(next_idx) {
            match node {
                RdNode::Text(s) => {
                    args_text.push_str(s);
                    nodes_consumed += 1;
                    // Stop if we've found the closing paren and this is the end of this usage line
                    if args_text.contains(')') && (s.ends_with(')') || s.contains('\n')) {
                        break;
                    }
                }
                RdNode::Special(ch) => {
                    args_text.push_str(special_char_to_string(*ch));
                    nodes_consumed += 1;
                }
                RdNode::LineBreak => {
                    // End of this usage line
                    break;
                }
                _ => break,
            }
        }

        // Trim only leading whitespace, preserve trailing for newlines
        let args_text_trimmed = args_text.trim_start();
        if !args_text_trimmed.starts_with('(') {
            return None;
        }

        // Find the matching closing paren
        let paren_end = find_matching_paren(args_text_trimmed)?;
        let args_content = &args_text_trimmed[1..paren_end];
        let trailing = &args_text_trimmed[paren_end + 1..];

        // Parse arguments (simple split by comma, respecting nested parens)
        let args = parse_function_args(args_content);

        // Format based on operator type
        let formatted = format_infix_call(generic, &args)?;

        Some(InfixFormatResult {
            text: format!("{}{}", formatted, trailing),
            nodes_consumed,
        })
    }
}

/// Result of formatting an infix method
struct InfixFormatResult {
    text: String,
    nodes_consumed: usize,
}

/// Check if a generic name is an infix operator
fn is_infix_operator(name: &str) -> bool {
    // Binary infix operators (with spaces)
    const PADDED_OPS: &[&str] = &[
        "+", "-", "*", "/", "==", "!=", "<", ">", "<=", ">=", "&", "|",
    ];
    // Infix operators without spaces
    const UNPADDED_OPS: &[&str] = &["^", "[", "[[", "$", ":", "::", ":::"];

    // User-defined infix operators: %...% (includes %% with length 2)
    if name.starts_with('%') && name.ends_with('%') && name.len() >= 2 {
        return true;
    }

    PADDED_OPS.contains(&name) || UNPADDED_OPS.contains(&name)
}

/// Check if operator should have spaces around it
fn is_padded_infix(name: &str) -> bool {
    const PADDED_OPS: &[&str] = &[
        "+", "-", "*", "/", "==", "!=", "<", ">", "<=", ">=", "&", "|",
    ];

    // User-defined infix operators also get spaces (includes %% with length 2)
    if name.starts_with('%') && name.ends_with('%') && name.len() >= 2 {
        return true;
    }

    PADDED_OPS.contains(&name)
}

/// Find the index of the matching closing parenthesis
fn find_matching_paren(s: &str) -> Option<usize> {
    if !s.starts_with('(') {
        return None;
    }

    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse function arguments, respecting nested parentheses
fn parse_function_args(args_content: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut depth = 0;

    for c in args_content.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                current_arg.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current_arg.push(c);
            }
            ',' if depth == 0 => {
                args.push(current_arg.trim().to_string());
                current_arg = String::new();
            }
            _ => {
                current_arg.push(c);
            }
        }
    }

    // Push the last argument
    let last = current_arg.trim().to_string();
    if !last.is_empty() {
        args.push(last);
    }

    args
}

/// Format an infix operator call in natural form
fn format_infix_call(operator: &str, args: &[String]) -> Option<String> {
    match operator {
        // Subscript operators
        "[" => {
            // x[i] or x[i, j, ...]
            if args.is_empty() {
                return None;
            }
            let obj = &args[0];
            let indices = &args[1..];
            if indices.is_empty() {
                Some(format!("{}[]", obj))
            } else {
                Some(format!("{}[{}]", obj, indices.join(", ")))
            }
        }
        "[[" => {
            // x[[i]] or x[[i, j]]
            if args.is_empty() {
                return None;
            }
            let obj = &args[0];
            let indices = &args[1..];
            if indices.is_empty() {
                Some(format!("{}[[]]", obj))
            } else {
                Some(format!("{}[[{}]]", obj, indices.join(", ")))
            }
        }
        "$" => {
            // x$name
            if args.len() != 2 {
                return None;
            }
            Some(format!("{}${}", args[0], args[1]))
        }
        // Namespace operators
        "::" | ":::" => {
            if args.len() != 2 {
                return None;
            }
            Some(format!("{}{}{}", args[0], operator, args[1]))
        }
        // Binary operators (padded and unpadded)
        _ => {
            // For binary operators, we need exactly 2 arguments
            if args.len() != 2 {
                return None;
            }
            if is_padded_infix(operator) {
                Some(format!("{} {} {}", args[0], operator, args[1]))
            } else {
                // Unpadded (like ^)
                Some(format!("{}{}{}", args[0], operator, args[1]))
            }
        }
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
    if s.is_empty() {
        return String::new();
    }

    // Check for leading and trailing whitespace
    let has_leading = s.chars().next().is_some_and(|c| c.is_whitespace());
    let has_trailing = s.chars().next_back().is_some_and(|c| c.is_whitespace());

    // Normalize internal whitespace (collapse multiple spaces to one)
    let normalized: String = s.split_whitespace().collect::<Vec<_>>().join(" ");

    if normalized.is_empty() {
        // Input was all whitespace - return a single space
        return " ".to_string();
    }

    // Restore leading/trailing spaces
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
}
