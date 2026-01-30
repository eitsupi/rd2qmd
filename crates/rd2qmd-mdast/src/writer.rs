//! mdast to Quarto Markdown writer
//!
//! Converts an mdast tree into a Quarto Markdown string.

use crate::mdast::{Align, Node, Root};
use serde::Serialize;

/// Options for the QMD writer
#[derive(Debug, Clone, Default)]
pub struct WriterOptions {
    /// Add YAML frontmatter
    pub frontmatter: Option<Frontmatter>,
    /// Use {r} instead of r for R code blocks
    pub quarto_code_blocks: bool,
}

/// YAML frontmatter content
#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    pub title: Option<String>,
    /// Page title for browser tab/SEO (pkgdown style: "<title> — <name>")
    pub pagetitle: Option<String>,
    pub format: Option<String>,
    /// Rd metadata (lifecycle, aliases, keywords, concepts)
    pub metadata: Option<RdMetadata>,
}

/// Metadata extracted from Rd documentation
///
/// This struct is shared between topic index generation and frontmatter output.
/// For frontmatter, the writer converts lifecycle to string via `as_str()`.
/// For topic index, serde serializes lifecycle as lowercase string.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RdMetadata {
    /// Lifecycle stage (deprecated, experimental, superseded, stable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<String>,
    /// Aliases for this topic (from \alias{})
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Keywords for this topic (from \keyword{})
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// Concepts for this topic (from \concept{})
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<String>,
    /// Source R files that generated this Rd file (from roxygen2 comments)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub source_files: Vec<String>,
}

/// Convert mdast to Quarto Markdown
pub fn mdast_to_qmd(root: &Root, options: &WriterOptions) -> String {
    let mut writer = Writer::new(options);
    writer.write_root(root)
}

/// QMD writer state
struct Writer<'a> {
    options: &'a WriterOptions,
    output: String,
    /// Whether we're at the start of a line
    at_line_start: bool,
}

impl<'a> Writer<'a> {
    fn new(options: &'a WriterOptions) -> Self {
        Self {
            options,
            output: String::new(),
            at_line_start: true,
        }
    }

    fn write_root(&mut self, root: &Root) -> String {
        // Write frontmatter if provided
        if let Some(fm) = &self.options.frontmatter {
            self.write_frontmatter(fm);
        }

        // Write children
        for (i, node) in root.children.iter().enumerate() {
            if i > 0 {
                self.ensure_blank_line();
            }
            self.write_node(node);
        }

        self.output.clone()
    }

    fn write_frontmatter(&mut self, fm: &Frontmatter) {
        self.output.push_str("---\n");
        if let Some(title) = &fm.title {
            self.output
                .push_str(&format!(r#"title: "{}""#, escape_yaml_string(title)));
            self.output.push('\n');
        }
        if let Some(pagetitle) = &fm.pagetitle {
            self.output
                .push_str(&format!(r#"pagetitle: "{}""#, escape_yaml_string(pagetitle)));
            self.output.push('\n');
        }
        if let Some(format) = &fm.format {
            self.output.push_str(&format!("format: {}\n", format));
        }
        // Write Rd metadata fields
        // TODO: Add configuration to control which metadata fields are output
        if let Some(metadata) = &fm.metadata {
            if let Some(lifecycle) = &metadata.lifecycle {
                self.output
                    .push_str(&format!("lifecycle: {}\n", lifecycle));
            }
            if !metadata.aliases.is_empty() {
                self.output.push_str("aliases:\n");
                for alias in &metadata.aliases {
                    self.output
                        .push_str(&format!(r#"  - "{}""#, escape_yaml_string(alias)));
                    self.output.push('\n');
                }
            }
            if !metadata.keywords.is_empty() {
                self.output.push_str("keywords:\n");
                for keyword in &metadata.keywords {
                    self.output
                        .push_str(&format!(r#"  - "{}""#, escape_yaml_string(keyword)));
                    self.output.push('\n');
                }
            }
            if !metadata.concepts.is_empty() {
                self.output.push_str("concepts:\n");
                for concept in &metadata.concepts {
                    self.output
                        .push_str(&format!(r#"  - "{}""#, escape_yaml_string(concept)));
                    self.output.push('\n');
                }
            }
            if !metadata.source_files.is_empty() {
                self.output.push_str("source-files:\n");
                for source_file in &metadata.source_files {
                    self.output
                        .push_str(&format!(r#"  - "{}""#, escape_yaml_string(source_file)));
                    self.output.push('\n');
                }
            }
        }
        self.output.push_str("---\n\n");
    }

    fn write_node(&mut self, node: &Node) {
        match node {
            Node::Heading(h) => self.write_heading(h),
            Node::Paragraph(p) => self.write_paragraph(p),
            Node::ThematicBreak => self.write_thematic_break(),
            Node::Blockquote(b) => self.write_blockquote(b),
            Node::List(l) => self.write_list(l),
            Node::ListItem(li) => self.write_list_item(li),
            Node::Code(c) => self.write_code(c),
            Node::Table(t) => self.write_table(t),
            Node::TableRow(_) => {}  // Handled by write_table
            Node::TableCell(_) => {} // Handled by write_table
            Node::DefinitionList(dl) => self.write_definition_list(dl),
            Node::DefinitionTerm(_) => {} // Handled by write_definition_list
            Node::DefinitionDescription(_) => {} // Handled by write_definition_list
            Node::Text(t) => self.output.push_str(&t.value),
            Node::Emphasis(e) => self.write_emphasis(e),
            Node::Strong(s) => self.write_strong(s),
            Node::InlineCode(c) => self.write_inline_code(c),
            Node::Break => self.write_break(),
            Node::Link(l) => self.write_link(l),
            Node::Image(img) => self.write_image(img),
            Node::Math(m) => self.write_math(m),
            Node::InlineMath(m) => self.write_inline_math(m),
            Node::Html(h) => self.output.push_str(&h.value),
        }
    }

    fn write_heading(&mut self, h: &crate::mdast::Heading) {
        self.ensure_newline();
        for _ in 0..h.depth {
            self.output.push('#');
        }
        self.output.push(' ');
        for child in &h.children {
            self.write_node(child);
        }
        self.output.push('\n');
        self.at_line_start = true;
    }

    fn write_paragraph(&mut self, p: &crate::mdast::Paragraph) {
        self.ensure_newline();
        for child in &p.children {
            self.write_node(child);
        }
        self.output.push('\n');
        self.at_line_start = true;
    }

    fn write_thematic_break(&mut self) {
        self.ensure_newline();
        self.output.push_str("---\n");
        self.at_line_start = true;
    }

    fn write_blockquote(&mut self, b: &crate::mdast::Blockquote) {
        self.ensure_newline();
        for child in &b.children {
            self.output.push_str("> ");
            self.write_node(child);
        }
    }

    fn write_list(&mut self, l: &crate::mdast::List) {
        self.write_list_at_indent(l, 0);
    }

    fn write_list_at_indent(&mut self, l: &crate::mdast::List, base_indent: usize) {
        self.ensure_newline();
        let mut num = l.start.unwrap_or(1);
        for child in &l.children {
            if let Node::ListItem(li) = child {
                // Write indent for this list item
                for _ in 0..base_indent {
                    self.output.push(' ');
                }
                if l.ordered {
                    self.output.push_str(&format!("{}. ", num));
                    num += 1;
                } else {
                    self.output.push_str("- ");
                }

                let item_indent = base_indent + 2;
                for (i, item_child) in li.children.iter().enumerate() {
                    match item_child {
                        Node::Paragraph(p) => {
                            if i > 0 {
                                // Subsequent paragraphs need indent
                                self.output.push('\n');
                                for _ in 0..item_indent {
                                    self.output.push(' ');
                                }
                            }
                            for c in &p.children {
                                self.write_node(c);
                            }
                        }
                        Node::List(nested) => {
                            // Nested list - write with increased indent
                            self.output.push('\n');
                            self.write_list_at_indent(nested, item_indent);
                            continue; // Skip the newline at the end since nested list handles it
                        }
                        _ => {
                            if i > 0 {
                                self.output.push('\n');
                                for _ in 0..item_indent {
                                    self.output.push(' ');
                                }
                            }
                            self.write_node(item_child);
                        }
                    }
                }
                self.output.push('\n');
            }
        }
        self.at_line_start = true;
    }

    fn write_list_item(&mut self, _li: &crate::mdast::ListItem) {
        // Handled by write_list
    }

    fn write_code(&mut self, c: &crate::mdast::Code) {
        self.ensure_newline();

        // Determine fence length: must be longer than any backtick sequence in content
        let fence_len = calculate_fence_length(&c.value);
        let fence: String = std::iter::repeat('`').take(fence_len).collect();

        self.output.push_str(&fence);
        if let Some(lang) = &c.lang {
            // Only use {r} for executable code blocks (Examples section)
            let is_executable = c.meta.as_deref() == Some("executable");
            if self.options.quarto_code_blocks && lang == "r" && is_executable {
                self.output.push_str("{r}");
            } else {
                self.output.push_str(lang);
            }
        }
        self.output.push('\n');
        self.output.push_str(&c.value);
        if !c.value.ends_with('\n') {
            self.output.push('\n');
        }
        self.output.push_str(&fence);
        self.output.push('\n');
        self.at_line_start = true;
    }

    fn write_table(&mut self, t: &crate::mdast::Table) {
        self.ensure_newline();

        let rows: Vec<&crate::mdast::TableRow> = t
            .children
            .iter()
            .filter_map(|n| {
                if let Node::TableRow(r) = n {
                    Some(r)
                } else {
                    None
                }
            })
            .collect();

        if rows.is_empty() {
            return;
        }

        // Calculate column widths
        let num_cols = rows.iter().map(|r| r.children.len()).max().unwrap_or(0);

        // Write header row
        if let Some(header) = rows.first() {
            self.write_table_row(header, num_cols);
        }

        // Write separator
        self.output.push('|');
        for i in 0..num_cols {
            let align = t.align.get(i).copied().flatten();
            match align {
                Some(Align::Left) => self.output.push_str(":---|"),
                Some(Align::Center) => self.output.push_str(":--:|"),
                Some(Align::Right) => self.output.push_str("---:|"),
                None => self.output.push_str("----|"),
            }
        }
        self.output.push('\n');

        // Write data rows
        for row in rows.iter().skip(1) {
            self.write_table_row(row, num_cols);
        }

        self.at_line_start = true;
    }

    fn write_table_row(&mut self, row: &crate::mdast::TableRow, num_cols: usize) {
        self.output.push('|');
        for (i, cell) in row.children.iter().enumerate() {
            if i >= num_cols {
                break;
            }
            if let Node::TableCell(c) = cell {
                self.output.push(' ');
                for child in &c.children {
                    self.write_node(child);
                }
                self.output.push_str(" |");
            }
        }
        // Fill missing cells
        for _ in row.children.len()..num_cols {
            self.output.push_str(" |");
        }
        self.output.push('\n');
    }

    fn write_definition_list(&mut self, dl: &crate::mdast::DefinitionList) {
        self.ensure_newline();

        let mut i = 0;
        while i < dl.children.len() {
            if let Node::DefinitionTerm(dt) = &dl.children[i] {
                // Write term
                for child in &dt.children {
                    self.write_node(child);
                }
                self.output.push('\n');

                // Write description(s)
                i += 1;
                while i < dl.children.len() {
                    if let Node::DefinitionDescription(dd) = &dl.children[i] {
                        // Check if description contains block elements
                        let has_block_elements = dd.children.iter().any(|c| {
                            matches!(
                                c,
                                Node::List(_)
                                    | Node::Code(_)
                                    | Node::Table(_)
                                    | Node::Blockquote(_)
                            )
                        });

                        if has_block_elements {
                            // Block elements need special handling with indentation
                            // Pandoc definition lists require blank line before nested blocks
                            // All content after first paragraph must be indented by 4 spaces
                            self.output.push_str(":   ");
                            let mut after_first = false;
                            for child in &dd.children {
                                match child {
                                    Node::Paragraph(p) => {
                                        if after_first {
                                            // Subsequent paragraphs need indentation
                                            self.output.push_str("    ");
                                        }
                                        for c in &p.children {
                                            self.write_node(c);
                                        }
                                        self.output.push_str("\n\n");
                                        after_first = true;
                                    }
                                    Node::List(l) => {
                                        // List items indented by 4 spaces
                                        self.write_indented_list(l, 4);
                                        self.output.push('\n');
                                        after_first = true;
                                    }
                                    _ => {
                                        if after_first {
                                            self.output.push_str("    ");
                                        }
                                        self.write_node(child);
                                        self.output.push('\n');
                                        after_first = true;
                                    }
                                }
                            }
                        } else {
                            // Simple inline content
                            self.output.push_str(":   ");
                            for child in &dd.children {
                                match child {
                                    Node::Paragraph(p) => {
                                        for c in &p.children {
                                            self.write_node(c);
                                        }
                                    }
                                    _ => self.write_node(child),
                                }
                            }
                            self.output.push('\n');
                        }
                        i += 1;
                    } else {
                        break;
                    }
                }
                self.output.push('\n');
            } else {
                i += 1;
            }
        }

        self.at_line_start = true;
    }

    fn write_indented_list(&mut self, l: &crate::mdast::List, indent: usize) {
        let indent_str = " ".repeat(indent);
        let mut num = l.start.unwrap_or(1);
        for child in &l.children {
            if let Node::ListItem(li) = child {
                self.output.push_str(&indent_str);
                if l.ordered {
                    self.output.push_str(&format!("{}. ", num));
                    num += 1;
                } else {
                    self.output.push_str("- ");
                }
                for (i, item_child) in li.children.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(&" ".repeat(indent + 2));
                    }
                    match item_child {
                        Node::Paragraph(p) => {
                            for c in &p.children {
                                self.write_node(c);
                            }
                        }
                        _ => self.write_node(item_child),
                    }
                }
                self.output.push('\n');
            }
        }
    }

    fn write_emphasis(&mut self, e: &crate::mdast::Emphasis) {
        self.output.push('*');
        for child in &e.children {
            self.write_node(child);
        }
        self.output.push('*');
    }

    fn write_strong(&mut self, s: &crate::mdast::Strong) {
        self.output.push_str("**");
        for child in &s.children {
            self.write_node(child);
        }
        self.output.push_str("**");
    }

    fn write_inline_code(&mut self, c: &crate::mdast::InlineCode) {
        // Add space before if the previous character is a backtick
        // This prevents `foo``bar` which CommonMark parses as a single code span
        if self.output.ends_with('`') {
            self.output.push(' ');
        }

        // Handle backticks in content
        let value = &c.value;
        if value.contains('`') {
            self.output.push_str("`` ");
            self.output.push_str(value);
            self.output.push_str(" ``");
        } else {
            self.output.push('`');
            self.output.push_str(value);
            self.output.push('`');
        }
    }

    fn write_break(&mut self) {
        self.output.push_str("  \n");
        self.at_line_start = true;
    }

    fn write_link(&mut self, l: &crate::mdast::Link) {
        self.output.push('[');
        for child in &l.children {
            self.write_node(child);
        }
        self.output.push_str("](");
        self.output.push_str(&l.url);
        if let Some(title) = &l.title {
            self.output.push_str(" \"");
            self.output.push_str(title);
            self.output.push('"');
        }
        self.output.push(')');
    }

    fn write_image(&mut self, img: &crate::mdast::Image) {
        self.output.push_str("![");
        self.output.push_str(&img.alt);
        self.output.push_str("](");
        self.output.push_str(&img.url);
        if let Some(title) = &img.title {
            self.output.push_str(" \"");
            self.output.push_str(title);
            self.output.push('"');
        }
        self.output.push(')');
    }

    fn write_math(&mut self, m: &crate::mdast::Math) {
        self.ensure_newline();
        self.output.push_str("$$\n");
        self.output.push_str(&m.value);
        if !m.value.ends_with('\n') {
            self.output.push('\n');
        }
        self.output.push_str("$$\n");
        self.at_line_start = true;
    }

    fn write_inline_math(&mut self, m: &crate::mdast::InlineMath) {
        self.output.push('$');
        self.output.push_str(&m.value);
        self.output.push('$');
    }

    // Helper methods

    fn ensure_newline(&mut self) {
        if !self.at_line_start && !self.output.is_empty() {
            self.output.push('\n');
            self.at_line_start = true;
        }
    }

    fn ensure_blank_line(&mut self) {
        self.ensure_newline();
        if !self.output.ends_with("\n\n") && !self.output.is_empty() {
            self.output.push('\n');
        }
    }
}

fn escape_yaml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Calculate the minimum fence length needed for a code block.
///
/// The fence must be longer than any sequence of consecutive backticks in the content.
/// Returns at least 3 (the minimum for a valid fenced code block).
fn calculate_fence_length(content: &str) -> usize {
    let mut max_backticks = 0;
    let mut current_run = 0;

    for c in content.chars() {
        if c == '`' {
            current_run += 1;
            max_backticks = max_backticks.max(current_run);
        } else {
            current_run = 0;
        }
    }

    // Fence must be at least 3 backticks and longer than any run in content
    3.max(max_backticks + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdast::*;

    #[test]
    fn test_heading() {
        let root = Root::new(vec![Node::heading(1, vec![Node::text("Title")])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert_eq!(qmd.trim(), "# Title");
    }

    #[test]
    fn test_heading_levels() {
        for depth in 1..=6 {
            let root = Root::new(vec![Node::heading(depth, vec![Node::text("Title")])]);
            let qmd = mdast_to_qmd(&root, &WriterOptions::default());
            let expected_prefix = "#".repeat(depth as usize);
            assert!(qmd.starts_with(&format!("{} Title", expected_prefix)));
        }
    }

    #[test]
    fn test_paragraph() {
        let root = Root::new(vec![Node::paragraph(vec![Node::text("Hello world")])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert_eq!(qmd.trim(), "Hello world");
    }

    #[test]
    fn test_multiple_paragraphs() {
        let root = Root::new(vec![
            Node::paragraph(vec![Node::text("First paragraph")]),
            Node::paragraph(vec![Node::text("Second paragraph")]),
        ]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("First paragraph"));
        assert!(qmd.contains("Second paragraph"));
        // Paragraphs should be separated by blank line
        assert!(qmd.contains("\n\n"));
    }

    #[test]
    fn test_code_block() {
        let root = Root::new(vec![Node::code(Some("r".to_string()), "x <- 1")]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("```r"));
        assert!(qmd.contains("x <- 1"));
        assert!(qmd.contains("```\n"));
    }

    #[test]
    fn test_code_block_no_language() {
        let root = Root::new(vec![Node::code(None, "some code")]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("```\n"));
        assert!(qmd.contains("some code"));
    }

    #[test]
    fn test_code_block_with_triple_backticks() {
        // Code containing triple backticks needs longer fence
        let code = "Here is a code block:\n```r\nx <- 1\n```";
        let root = Root::new(vec![Node::code(Some("markdown".to_string()), code)]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());

        // Should use 4 backticks since content has 3
        assert!(qmd.contains("````markdown"));
        assert!(qmd.contains("````\n") || qmd.ends_with("````"));
    }

    #[test]
    fn test_code_block_with_many_backticks() {
        // Code containing 5 backticks needs 6 for fence
        let code = "`````";
        let root = Root::new(vec![Node::code(None, code)]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());

        // Should use 6 backticks
        assert!(qmd.contains("``````\n"));
    }

    #[test]
    fn test_calculate_fence_length() {
        // No backticks -> 3
        assert_eq!(calculate_fence_length("hello"), 3);

        // 1 backtick -> 3
        assert_eq!(calculate_fence_length("hello ` world"), 3);

        // 2 backticks -> 3
        assert_eq!(calculate_fence_length("hello `` world"), 3);

        // 3 backticks -> 4
        assert_eq!(calculate_fence_length("```"), 4);

        // 3 backticks in middle -> 4
        assert_eq!(calculate_fence_length("hello\n```r\nx <- 1\n```"), 4);

        // 5 backticks -> 6
        assert_eq!(calculate_fence_length("`````"), 6);
    }

    #[test]
    fn test_quarto_code_block() {
        // Executable code block (Examples section) uses {r}
        let root = Root::new(vec![Node::code_with_meta(
            Some("r".to_string()),
            Some("executable".to_string()),
            "x <- 1",
        )]);
        let opts = WriterOptions {
            quarto_code_blocks: true,
            ..Default::default()
        };
        let qmd = mdast_to_qmd(&root, &opts);
        assert!(qmd.contains("```{r}"));

        // Non-executable code block (Usage section) uses plain r
        let root2 = Root::new(vec![Node::code(Some("r".to_string()), "foo(x)")]);
        let qmd2 = mdast_to_qmd(&root2, &opts);
        assert!(qmd2.contains("```r"));
        assert!(!qmd2.contains("```{r}"));
    }

    #[test]
    fn test_inline_code() {
        let root = Root::new(vec![Node::paragraph(vec![
            Node::text("Use "),
            Node::inline_code("foo()"),
            Node::text(" here"),
        ])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("`foo()`"));
    }

    #[test]
    fn test_inline_code_with_backticks() {
        let root = Root::new(vec![Node::paragraph(vec![Node::inline_code(
            "code with ` backtick",
        )])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("`` code with ` backtick ``"));
    }

    #[test]
    fn test_consecutive_inline_codes() {
        // Consecutive inline codes need a space between them
        // Without a space, `foo``bar` is parsed as a single code span in CommonMark
        let root = Root::new(vec![Node::paragraph(vec![
            Node::inline_code("foo"),
            Node::inline_code("bar"),
        ])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        // Should produce "`foo` `bar`" not "`foo``bar`"
        assert!(qmd.contains("`foo` `bar`"));
        assert!(!qmd.contains("`foo``bar`"));
    }

    #[test]
    fn test_emphasis_and_strong() {
        let root = Root::new(vec![Node::paragraph(vec![
            Node::emphasis(vec![Node::text("italic")]),
            Node::text(" and "),
            Node::strong(vec![Node::text("bold")]),
        ])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("*italic*"));
        assert!(qmd.contains("**bold**"));
    }

    #[test]
    fn test_nested_emphasis() {
        let root = Root::new(vec![Node::paragraph(vec![Node::strong(vec![
            Node::text("bold "),
            Node::emphasis(vec![Node::text("and italic")]),
        ])])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("**bold *and italic***"));
    }

    #[test]
    fn test_link() {
        let root = Root::new(vec![Node::paragraph(vec![Node::link(
            "https://example.com",
            vec![Node::text("Example")],
        )])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("[Example](https://example.com)"));
    }

    #[test]
    fn test_link_with_title() {
        let root = Root::new(vec![Node::paragraph(vec![Node::link_with_title(
            "https://example.com",
            "Example Site",
            vec![Node::text("Example")],
        )])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("[Example](https://example.com \"Example Site\")"));
    }

    #[test]
    fn test_image() {
        let root = Root::new(vec![Node::paragraph(vec![Node::image(
            "image.png",
            "An image",
        )])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("![An image](image.png)"));
    }

    #[test]
    fn test_image_with_title() {
        let root = Root::new(vec![Node::paragraph(vec![Node::image_with_title(
            "image.png",
            "An image",
            "Image Title",
        )])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("![An image](image.png \"Image Title\")"));
    }

    #[test]
    fn test_unordered_list() {
        let root = Root::new(vec![Node::list(
            false,
            vec![
                Node::list_item(vec![Node::paragraph(vec![Node::text("A")])]),
                Node::list_item(vec![Node::paragraph(vec![Node::text("B")])]),
            ],
        )]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("- A"));
        assert!(qmd.contains("- B"));
    }

    #[test]
    fn test_ordered_list() {
        let root = Root::new(vec![Node::list(
            true,
            vec![
                Node::list_item(vec![Node::paragraph(vec![Node::text("First")])]),
                Node::list_item(vec![Node::paragraph(vec![Node::text("Second")])]),
            ],
        )]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("1. First"));
        assert!(qmd.contains("2. Second"));
    }

    #[test]
    fn test_ordered_list_custom_start() {
        let root = Root::new(vec![Node::ordered_list_from(
            5,
            vec![
                Node::list_item(vec![Node::paragraph(vec![Node::text("Five")])]),
                Node::list_item(vec![Node::paragraph(vec![Node::text("Six")])]),
            ],
        )]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("5. Five"));
        assert!(qmd.contains("6. Six"));
    }

    #[test]
    fn test_nested_list() {
        let root = Root::new(vec![Node::list(
            false,
            vec![Node::list_item(vec![
                Node::paragraph(vec![Node::text("Parent")]),
                Node::list(
                    false,
                    vec![Node::list_item(vec![Node::paragraph(vec![Node::text(
                        "Child",
                    )])])],
                ),
            ])],
        )]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("- Parent"));
        assert!(qmd.contains("  - Child"));
    }

    #[test]
    fn test_table() {
        let root = Root::new(vec![Node::table(
            vec![Some(Align::Left), Some(Align::Right)],
            vec![
                Node::table_row(vec![
                    Node::table_cell(vec![Node::text("Name")]),
                    Node::table_cell(vec![Node::text("Value")]),
                ]),
                Node::table_row(vec![
                    Node::table_cell(vec![Node::text("foo")]),
                    Node::table_cell(vec![Node::text("1")]),
                ]),
            ],
        )]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("| Name | Value |"));
        assert!(qmd.contains("|:---|---:|"));
        assert!(qmd.contains("| foo | 1 |"));
    }

    #[test]
    fn test_table_center_align() {
        let root = Root::new(vec![Node::table(
            vec![Some(Align::Center)],
            vec![
                Node::table_row(vec![Node::table_cell(vec![Node::text("Header")])]),
                Node::table_row(vec![Node::table_cell(vec![Node::text("Data")])]),
            ],
        )]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("|:--:|"));
    }

    #[test]
    fn test_definition_list() {
        let root = Root::new(vec![Node::definition_list(vec![
            Node::definition_term(vec![Node::text("Term")]),
            Node::definition_description(vec![Node::paragraph(vec![Node::text("Definition")])]),
        ])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("Term"));
        assert!(qmd.contains(":   Definition"));
    }

    #[test]
    fn test_math() {
        let root = Root::new(vec![
            Node::paragraph(vec![Node::inline_math("x^2")]),
            Node::math("E = mc^2"),
        ]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("$x^2$"));
        assert!(qmd.contains("$$\nE = mc^2\n$$"));
    }

    #[test]
    fn test_blockquote() {
        let root = Root::new(vec![Node::blockquote(vec![Node::paragraph(vec![
            Node::text("Quote"),
        ])])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("> Quote"));
    }

    #[test]
    fn test_thematic_break() {
        let root = Root::new(vec![
            Node::paragraph(vec![Node::text("Before")]),
            Node::thematic_break(),
            Node::paragraph(vec![Node::text("After")]),
        ]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("Before"));
        assert!(qmd.contains("\n---\n"));
        assert!(qmd.contains("After"));
    }

    #[test]
    fn test_line_break() {
        let root = Root::new(vec![Node::paragraph(vec![
            Node::text("Line 1"),
            Node::line_break(),
            Node::text("Line 2"),
        ])]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("Line 1  \nLine 2"));
    }

    #[test]
    fn test_html() {
        let root = Root::new(vec![Node::html("<div>Raw HTML</div>")]);
        let qmd = mdast_to_qmd(&root, &WriterOptions::default());
        assert!(qmd.contains("<div>Raw HTML</div>"));
    }

    #[test]
    fn test_frontmatter() {
        let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
        let opts = WriterOptions {
            frontmatter: Some(Frontmatter {
                title: Some("My Document".to_string()),
                pagetitle: None,
                format: Some("html".to_string()),
                metadata: None,
            }),
            ..Default::default()
        };
        let qmd = mdast_to_qmd(&root, &opts);
        assert!(qmd.starts_with("---\n"));
        assert!(qmd.contains(r#"title: "My Document""#));
        assert!(qmd.contains("format: html"));
    }

    #[test]
    fn test_frontmatter_with_pagetitle() {
        let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
        let opts = WriterOptions {
            frontmatter: Some(Frontmatter {
                title: Some("Order rows using column values".to_string()),
                pagetitle: Some("Order rows using column values — arrange".to_string()),
                format: None,
                metadata: None,
            }),
            ..Default::default()
        };
        let qmd = mdast_to_qmd(&root, &opts);
        assert!(qmd.starts_with("---\n"));
        assert!(qmd.contains(r#"title: "Order rows using column values""#));
        assert!(qmd.contains(r#"pagetitle: "Order rows using column values — arrange""#));
    }

    #[test]
    fn test_frontmatter_escaping() {
        let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
        let opts = WriterOptions {
            frontmatter: Some(Frontmatter {
                title: Some(r#"Title with "quotes" and \backslash"#.to_string()),
                pagetitle: None,
                format: None,
                metadata: None,
            }),
            ..Default::default()
        };
        let qmd = mdast_to_qmd(&root, &opts);
        assert!(qmd.contains(r#"title: "Title with \"quotes\" and \\backslash""#));
    }

    #[test]
    fn test_frontmatter_with_metadata() {
        let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
        let opts = WriterOptions {
            frontmatter: Some(Frontmatter {
                title: Some("My Function".to_string()),
                pagetitle: None,
                format: None,
                metadata: Some(RdMetadata {
                    lifecycle: Some("deprecated".to_string()),
                    aliases: vec!["my_func".to_string(), "MyFunc".to_string()],
                    keywords: vec!["misc".to_string(), "internal".to_string()],
                    concepts: vec!["data manipulation".to_string()],
                    source_files: vec![],
                }),
            }),
            ..Default::default()
        };
        let qmd = mdast_to_qmd(&root, &opts);
        assert!(qmd.starts_with("---\n"));
        assert!(qmd.contains(r#"title: "My Function""#));
        assert!(qmd.contains("lifecycle: deprecated"));
        assert!(qmd.contains("aliases:"));
        assert!(qmd.contains(r#"  - "my_func""#));
        assert!(qmd.contains(r#"  - "MyFunc""#));
        assert!(qmd.contains("keywords:"));
        assert!(qmd.contains(r#"  - "misc""#));
        assert!(qmd.contains(r#"  - "internal""#));
        assert!(qmd.contains("concepts:"));
        assert!(qmd.contains(r#"  - "data manipulation""#));
    }

    #[test]
    fn test_frontmatter_metadata_empty_vectors_omitted() {
        let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
        let opts = WriterOptions {
            frontmatter: Some(Frontmatter {
                title: Some("Function".to_string()),
                pagetitle: None,
                format: None,
                metadata: Some(RdMetadata {
                    lifecycle: Some("stable".to_string()),
                    aliases: vec![],
                    keywords: vec![],
                    concepts: vec![],
                    source_files: vec![],
                }),
            }),
            ..Default::default()
        };
        let qmd = mdast_to_qmd(&root, &opts);
        assert!(qmd.contains("lifecycle: stable"));
        // Empty vectors should not appear
        assert!(!qmd.contains("aliases:"));
        assert!(!qmd.contains("keywords:"));
        assert!(!qmd.contains("concepts:"));
        assert!(!qmd.contains("source-files:"));
    }

    #[test]
    fn test_frontmatter_with_source_files() {
        let root = Root::new(vec![Node::paragraph(vec![Node::text("Content")])]);
        let opts = WriterOptions {
            frontmatter: Some(Frontmatter {
                title: Some("coord_map".to_string()),
                pagetitle: None,
                format: None,
                metadata: Some(RdMetadata {
                    lifecycle: None,
                    aliases: vec![],
                    keywords: vec![],
                    concepts: vec![],
                    source_files: vec!["R/coord-map.R".to_string(), "R/coord-quickmap.R".to_string()],
                }),
            }),
            ..Default::default()
        };
        let qmd = mdast_to_qmd(&root, &opts);
        assert!(qmd.contains("source-files:\n"));
        assert!(qmd.contains(r#"  - "R/coord-map.R""#));
        assert!(qmd.contains(r#"  - "R/coord-quickmap.R""#));
    }
}
