//! Rd AST to mdast conversion
//!
//! Converts an Rd document into an mdast tree for Markdown output.

use rd_parser::{
    DescribeItem, FigureOptions, RdDocument, RdNode, RdSection, SectionTag, SpecialChar,
};
#[cfg(feature = "roxygen")]
use crate::roxygen_code_block::try_match_roxygen_code_block;
use rd2qmd_mdast::{
    Align, DefinitionDescription, DefinitionList, DefinitionTerm, Html, Image, Node, Root, Table,
    TableCell, TableRow,
};
use std::collections::HashMap;
use tabled::settings::Style;
use tabled::settings::style::HorizontalLine;

/// Format for the Arguments section output
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ArgumentsFormat {
    /// Pipe table - limited to inline content in cells
    PipeTable,
    /// Pandoc grid table (default) - supports block elements (lists, paragraphs) in cells
    #[default]
    GridTable,
}

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
    /// Format for the Arguments section
    /// GfmTable (default): GFM pipe table, limited to inline content
    /// GridTable: Pandoc grid table, supports block elements in cells
    pub arguments_format: ArgumentsFormat,
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
            arguments_format: ArgumentsFormat::default(),
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
                RdNode::DontDiff(children) => {
                    // \dontdiff{} marks code whose output shouldn't be diff-checked.
                    // The code itself should still execute normally, so treat like regular code.
                    has_executable = true;
                    current_code.push_str(&self.extract_text(children));
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

    fn convert_arguments(&mut self, content: &[RdNode]) -> Vec<Node> {
        match self.options.arguments_format {
            ArgumentsFormat::PipeTable => self.convert_arguments_pipe(content),
            ArgumentsFormat::GridTable => self.convert_arguments_grid(content),
        }
    }

    /// Convert arguments to pipe table format.
    /// Pipe tables cannot contain block elements (lists, multiple paragraphs).
    /// Workaround: use <br> for line breaks, flatten nested lists with bullet markers.
    fn convert_arguments_pipe(&mut self, content: &[RdNode]) -> Vec<Node> {
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

    /// Convert arguments to Pandoc grid table format.
    /// Grid tables support block elements (lists, paragraphs) within cells.
    fn convert_arguments_grid(&mut self, content: &[RdNode]) -> Vec<Node> {
        use tabled::builder::Builder;

        let mut builder = Builder::default();
        builder.push_record(["Argument", "Description"]);

        for node in content {
            if let RdNode::Item { label, content } = node
                && let Some(label_nodes) = label
            {
                // Argument name with backticks for inline code
                let term_text = self.extract_text(label_nodes);
                let arg_text = format!("`{}`", term_text.trim());

                // Convert description to Markdown text for grid table
                let desc_text = self.convert_to_markdown_text(content);

                builder.push_record([arg_text, desc_text]);
            }
        }

        let mut table = builder.build();

        // Check if table has any data rows
        if table.count_rows() <= 1 {
            // Only header, no data rows - fall back to regular content
            return self.convert_content(content);
        }

        // Apply grid table style with = separator after header
        let grid_style = Style::ascii().horizontals([(
            1,
            HorizontalLine::new('=')
                .left('+')
                .right('+')
                .intersection('+'),
        )]);
        let grid_table = table.with(grid_style).to_string();

        // Output as raw text (will be rendered as grid table by Pandoc)
        vec![Node::Html(Html { value: grid_table })]
    }

    /// Convert RdNode content to Markdown text for use in grid table cells.
    ///
    /// # Why this exists (separate from the main writer)
    ///
    /// Grid tables are built using the `tabled` library, which operates on raw strings.
    /// Unlike the main mdast writer (`rd2qmd_mdast::writer`), which writes to a single
    /// global output string, we need to convert AST subtrees to standalone markdown strings
    /// for each table cell.
    ///
    /// The main writer cannot easily produce markdown for a subtree because:
    /// 1. It maintains global state (line position, blank line tracking)
    /// 2. It writes directly to an output buffer, not returning strings
    ///
    /// Ideally, we could refactor to either:
    /// - Make the writer able to serialize subtrees to strings
    /// - Use a table library that accepts AST nodes directly
    ///
    /// For now, `nodes_to_markdown` and `inline_nodes_to_markdown` exist as a separate
    /// code path specifically for grid table cell content.
    fn convert_to_markdown_text(&mut self, content: &[RdNode]) -> String {
        let nodes = self.convert_content(content);
        self.nodes_to_markdown(&nodes)
    }

    /// Convert mdast nodes to Markdown text string (for grid table cells).
    /// See `convert_to_markdown_text` for why this exists.
    fn nodes_to_markdown(&self, nodes: &[Node]) -> String {
        let mut result = String::new();

        for (i, node) in nodes.iter().enumerate() {
            if i > 0 {
                result.push_str("\n\n");
            }

            match node {
                Node::Paragraph(p) => {
                    result.push_str(&self.inline_nodes_to_markdown(&p.children));
                }
                Node::List(l) => {
                    // Lists need a blank line before them in grid tables
                    if i > 0 && !result.ends_with("\n\n") {
                        result.push('\n');
                    }
                    for (j, item) in l.children.iter().enumerate() {
                        if j > 0 {
                            result.push('\n');
                        }
                        if let Node::ListItem(li) = item {
                            let marker = if l.ordered {
                                format!("{}. ", j + 1)
                            } else {
                                "- ".to_string()
                            };
                            result.push_str(&marker);
                            // Get first paragraph content
                            for child in &li.children {
                                if let Node::Paragraph(p) = child {
                                    result.push_str(&self.inline_nodes_to_markdown(&p.children));
                                    break;
                                }
                            }
                        }
                    }
                }
                Node::Code(c) => {
                    result.push_str("```");
                    if let Some(lang) = &c.lang {
                        result.push_str(lang);
                    }
                    result.push('\n');
                    result.push_str(&c.value);
                    result.push_str("\n```");
                }
                _ => {
                    // For other nodes, try to extract text
                    if let Some(text) = self.node_to_text(node) {
                        result.push_str(&text);
                    }
                }
            }
        }

        result
    }

    /// Convert inline mdast nodes to Markdown text (for grid table cells).
    ///
    /// This handles inline elements like text, code, emphasis, links, and images.
    /// See `convert_to_markdown_text` for why this separate code path exists.
    fn inline_nodes_to_markdown(&self, nodes: &[Node]) -> String {
        let mut result = String::new();

        for node in nodes {
            match node {
                Node::Text(t) => result.push_str(&t.value),
                Node::InlineCode(c) => {
                    result.push('`');
                    result.push_str(&c.value);
                    result.push('`');
                }
                Node::Emphasis(e) => {
                    result.push('*');
                    result.push_str(&self.inline_nodes_to_markdown(&e.children));
                    result.push('*');
                }
                Node::Strong(s) => {
                    result.push_str("**");
                    result.push_str(&self.inline_nodes_to_markdown(&s.children));
                    result.push_str("**");
                }
                Node::Link(l) => {
                    result.push('[');
                    result.push_str(&self.inline_nodes_to_markdown(&l.children));
                    result.push_str("](");
                    result.push_str(&l.url);
                    result.push(')');
                }
                Node::InlineMath(m) => {
                    result.push('$');
                    result.push_str(&m.value);
                    result.push('$');
                }
                Node::Image(img) => {
                    result.push_str("![");
                    result.push_str(&img.alt);
                    result.push_str("](");
                    result.push_str(&img.url);
                    if let Some(title) = &img.title {
                        result.push_str(" \"");
                        result.push_str(title);
                        result.push('"');
                    }
                    result.push(')');
                }
                Node::Break => result.push_str("  \n"),
                Node::Html(h) => result.push_str(&h.value),
                _ => {
                    if let Some(text) = self.node_to_text(node) {
                        result.push_str(&text);
                    }
                }
            }
        }

        result
    }

    /// Extract plain text from a node (for fallback).
    fn node_to_text(&self, node: &Node) -> Option<String> {
        match node {
            Node::Text(t) => Some(t.value.clone()),
            Node::InlineCode(c) => Some(format!("`{}`", c.value)),
            Node::Paragraph(p) => Some(self.inline_nodes_to_markdown(&p.children)),
            _ => None,
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
                result.push(Node::Html(Html {
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
                            result.push(Node::Html(Html {
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
        let mut i = 0;

        while i < nodes.len() {
            // Try to match roxygen2 markdown code block pattern
            #[cfg(feature = "roxygen")]
            if let Some(code_block) = try_match_roxygen_code_block(&nodes[i..]) {
                self.flush_paragraph(&mut current_para, &mut result);
                result.push(Node::code(code_block.language, code_block.code));
                i += code_block.nodes_consumed;
                continue;
            }

            let node = &nodes[i];
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
                    for (j, part) in parts.iter().enumerate() {
                        if j > 0 {
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
            i += 1;
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
                        // Package name is already parsed by the parser (no colon)
                        // For \link[pkg]{topic}: text is None, use pkg::topic format
                        // For \link[pkg:bar]{foo}: text is Some, use the display_text
                        let display = if text.is_some() {
                            display_text
                        } else {
                            format!("{}::{}", pkg, topic)
                        };

                        // Check if we have a URL for this external package
                        if let Some(base_url) = self
                            .options
                            .external_package_urls
                            .as_ref()
                            .and_then(|map| map.get(pkg.as_str()))
                        {
                            let url =
                                format!("{}/{}.html", base_url.trim_end_matches('/'), topic);
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
            RdNode::Doi(id) => {
                let url = format!("https://doi.org/{}", id);
                let display = format!("doi:{}", id);
                Some(Node::link(url, vec![Node::text(display)]))
            }
            RdNode::LinkS4Class { package, classname } => {
                // Display text includes -class suffix for clarity
                let display = if let Some(pkg) = package {
                    format!("{}::{}-class", pkg, classname)
                } else {
                    format!("{}-class", classname)
                };

                match (package, &self.options.link_extension) {
                    // External package link
                    (Some(pkg), _) => {
                        if let Some(base_url) = self
                            .options
                            .external_package_urls
                            .as_ref()
                            .and_then(|map| map.get(pkg.as_str()))
                        {
                            // Link to classname-class topic
                            let url = format!(
                                "{}/{}-class.html",
                                base_url.trim_end_matches('/'),
                                classname
                            );
                            Some(Node::link(url, vec![Node::inline_code(display)]))
                        } else {
                            Some(Node::inline_code(display))
                        }
                    }
                    // Internal link with extension configured
                    (None, Some(ext)) => {
                        // The topic name is classname-class
                        let topic = format!("{}-class", classname);
                        if let Some(target_file) = self
                            .options
                            .alias_map
                            .as_ref()
                            .and_then(|map| map.get(&topic))
                        {
                            let url = format!("{}.{}", target_file, ext);
                            Some(Node::link(url, vec![Node::inline_code(display)]))
                        } else if let Some(pattern) = &self.options.unresolved_link_url {
                            let url = pattern.replace("{topic}", &topic);
                            Some(Node::link(url, vec![Node::inline_code(display)]))
                        } else {
                            Some(Node::inline_code(display))
                        }
                    }
                    // No extension - just inline code
                    (None, None) => Some(Node::inline_code(display)),
                }
            }
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
            RdNode::Out(html) => Some(Node::Html(Html {
                value: html.clone(),
            })),
            RdNode::Figure { file, options } => {
                let alt = match options {
                    Some(FigureOptions::AltText(text)) => text.clone(),
                    Some(FigureOptions::ExpertOptions(attrs)) => {
                        Self::extract_alt_from_attrs(attrs).unwrap_or_else(|| file.clone())
                    }
                    None => file.clone(),
                };
                Some(Node::Image(Image {
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
            RdNode::S3Method { generic, class: _ } => Some(Node::text(format!("{}()", generic))),
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

    fn convert_describe(&self, items: &[DescribeItem]) -> Node {
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

    /// Extract alt text from HTML attributes string (Expert form).
    ///
    /// This function handles the expert form where the parser has already stripped
    /// the "options:" prefix, leaving just the HTML/LaTeX attributes string.
    ///
    /// Reference: https://cran.r-project.org/doc/manuals/r-devel/R-exts.html#Figures
    fn extract_alt_from_attrs(attrs: &str) -> Option<String> {
        if attrs.is_empty() {
            return None;
        }
        // Try single quotes: alt='...'
        if let Some(start) = attrs.find("alt='") {
            let after_quote = &attrs[start + 5..];
            if let Some(end) = after_quote.find('\'') {
                return Some(after_quote[..end].to_string());
            }
        }
        // Try double quotes: alt="..."
        if let Some(start) = attrs.find("alt=\"") {
            let after_quote = &attrs[start + 5..];
            if let Some(end) = after_quote.find('"') {
                return Some(after_quote[..end].to_string());
            }
        }
        // No alt attribute found
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
                RdNode::S3Method { generic, class } => {
                    // S3 method: same as Method, add comment like pkgdown
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
                RdNode::LinkS4Class {
                    package,
                    classname,
                } => {
                    if let Some(pkg) = package {
                        result.push_str(&format!("{}::{}", pkg, classname));
                    } else {
                        result.push_str(classname);
                    }
                }
                RdNode::Doi(id) => {
                    result.push_str(&format!("doi:{}", id));
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
mod tests;
