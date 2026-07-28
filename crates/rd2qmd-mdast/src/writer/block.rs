//! Block-level node writers.

use super::Writer;
use crate::mdast::Align;
use crate::mdast::Node;

impl<'a> Writer<'a> {
    pub(super) fn write_heading(&mut self, h: &crate::mdast::Heading) {
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

    pub(super) fn write_paragraph(&mut self, p: &crate::mdast::Paragraph) {
        self.ensure_newline();
        for child in &p.children {
            self.write_node(child);
        }
        self.output.push('\n');
        self.at_line_start = true;
    }

    pub(super) fn write_thematic_break(&mut self) {
        self.ensure_newline();
        self.output.push_str("---\n");
        self.at_line_start = true;
    }

    pub(super) fn write_blockquote(&mut self, b: &crate::mdast::Blockquote) {
        self.ensure_newline();
        for child in &b.children {
            self.output.push_str("> ");
            self.write_node(child);
        }
    }

    pub(super) fn write_list(&mut self, l: &crate::mdast::List) {
        self.write_list_at_indent(l, 0);
    }

    fn write_list_at_indent(&mut self, l: &crate::mdast::List, base_indent: usize) {
        self.ensure_newline();
        // A list is "loose" when any item contains more than one block child.
        // Loose lists require blank lines between items and between blocks within an item.
        let is_loose = l.children.iter().any(|child| {
            if let Node::ListItem(li) = child {
                li.children.len() > 1
            } else {
                false
            }
        });
        let mut num = l.start.unwrap_or(1);
        for child in &l.children {
            if let Node::ListItem(li) = child {
                // Write indent for this list item
                for _ in 0..base_indent {
                    self.output.push(' ');
                }
                // Build the marker first so item_indent matches the actual marker width.
                // "- " is always 2 chars, but "10. " is 4 — a fixed +2 would mis-indent
                // continuation lines for ordered lists with wide numbers.
                let marker = if l.ordered {
                    let m = format!("{}. ", num);
                    num += 1;
                    m
                } else {
                    "- ".to_string()
                };
                self.output.push_str(&marker);
                let item_indent = base_indent + marker.len();

                for (i, item_child) in li.children.iter().enumerate() {
                    match item_child {
                        Node::Paragraph(p) => {
                            if i > 0 {
                                // Subsequent paragraphs in a loose item need a blank line
                                self.output.push('\n');
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
                            // Nested list - write with increased indent.
                            // Do not manually pre-indent here; write_list_at_indent already
                            // emits base_indent spaces per item, so pre-indenting would double it.
                            self.output.push('\n');
                            if is_loose {
                                self.output.push('\n');
                            }
                            self.write_list_at_indent(nested, item_indent);
                            continue; // Skip the newline at the end since nested list handles it
                        }
                        _ => {
                            let indent_str = " ".repeat(item_indent);
                            if i > 0 {
                                self.output.push('\n');
                                self.output.push('\n');
                                self.output.push_str(&indent_str);
                            }
                            // Capture the block output, then re-indent every continuation line
                            // so multi-line blocks (code fences, tables) stay inside the item.
                            let start = self.output.len();
                            self.write_node(item_child);
                            let raw = self.output[start..].to_string();
                            self.output.truncate(start);
                            for (j, part) in raw.split('\n').enumerate() {
                                if j > 0 {
                                    self.output.push('\n');
                                    if !part.is_empty() {
                                        self.output.push_str(&indent_str);
                                    }
                                }
                                self.output.push_str(part);
                            }
                        }
                    }
                }
                self.output.push('\n');
                if is_loose {
                    self.output.push('\n');
                }
            }
        }
        self.at_line_start = true;
    }

    pub(super) fn write_list_item(&mut self, _li: &crate::mdast::ListItem) {
        // Handled by write_list
    }

    pub(super) fn write_code(&mut self, c: &crate::mdast::Code) {
        self.ensure_newline();

        // Determine fence length: must be longer than any backtick sequence in content
        let fence_len = calculate_fence_length(&c.value);
        let fence = "`".repeat(fence_len);

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

    pub(super) fn write_table(&mut self, t: &crate::mdast::Table) {
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

    pub(super) fn write_definition_list(&mut self, dl: &crate::mdast::DefinitionList) {
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
                        let has_block_elements = dd.children.len() > 1
                            || dd.children.iter().any(|c| {
                                matches!(
                                    c,
                                    Node::List(_)
                                        | Node::Code(_)
                                        | Node::Table(_)
                                        | Node::Blockquote(_)
                                        | Node::Math(_)
                                        | Node::DefinitionList(_)
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
                                        self.write_reindented_block(child, 4);
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

    /// Render `node`, then re-indent every line after the first by `indent`
    /// spaces. Used to keep multi-line block children (code fences, nested
    /// lists, tables, ...) properly indented when nested inside a definition
    /// description or a list item. The caller is responsible for indenting
    /// the first line before calling this.
    fn write_reindented_block(&mut self, node: &Node, indent: usize) {
        let indent_str = " ".repeat(indent);
        let start = self.output.len();
        self.write_node(node);
        let raw = self.output[start..].to_string();
        self.output.truncate(start);

        for (i, line) in raw.split('\n').enumerate() {
            if i > 0 {
                self.output.push('\n');
                if !line.is_empty() {
                    self.output.push_str(&indent_str);
                }
            }
            self.output.push_str(line);
        }
    }

    fn write_indented_list(&mut self, l: &crate::mdast::List, indent: usize) {
        let indent_str = " ".repeat(indent);
        let mut num = l.start.unwrap_or(1);
        for child in &l.children {
            if let Node::ListItem(li) = child {
                self.output.push_str(&indent_str);
                // Build the marker first so item_indent matches the actual marker width.
                // "- " is always 2 chars, but "10. " is 4 — a fixed +2 would mis-indent
                // continuation lines for ordered lists with wide numbers.
                let marker = if l.ordered {
                    let m = format!("{}. ", num);
                    num += 1;
                    m
                } else {
                    "- ".to_string()
                };
                self.output.push_str(&marker);
                let item_indent = indent + marker.len();
                let item_indent_str = " ".repeat(item_indent);
                for (i, item_child) in li.children.iter().enumerate() {
                    match item_child {
                        Node::Paragraph(p) => {
                            if i > 0 {
                                self.output.push('\n');
                                self.output.push('\n');
                                self.output.push_str(&item_indent_str);
                            }
                            for c in &p.children {
                                self.write_node(c);
                            }
                        }
                        _ => {
                            // Block children (code fences, nested lists, tables, ...) must
                            // start on their own line, separated from preceding content by
                            // a blank line, with every continuation line re-indented to
                            // `item_indent` so they stay inside the list item.
                            self.output.push('\n');
                            if i > 0 {
                                self.output.push('\n');
                            }
                            self.output.push_str(&item_indent_str);
                            self.write_reindented_block(item_child, item_indent);
                        }
                    }
                }
                // Block children (e.g. a code fence) already end with their own
                // trailing newline; avoid stacking a second one here, which would
                // otherwise turn into a spurious extra blank line once the caller
                // (e.g. write_definition_list) adds its own separator newline.
                if !self.output.ends_with('\n') {
                    self.output.push('\n');
                }
            }
        }
    }
}

/// Calculate the minimum fence length needed for a code block.
///
/// The fence must be longer than any sequence of consecutive backticks in the content.
/// Returns at least 3 (the minimum for a valid fenced code block).
pub(super) fn calculate_fence_length(content: &str) -> usize {
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
