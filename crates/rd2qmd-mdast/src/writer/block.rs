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
        if !h.children.is_empty() {
            self.output.push(' ');
        }
        self.write_inlines(&h.children, None);
        self.output.push('\n');
        self.at_line_start = true;
    }

    pub(super) fn write_paragraph(&mut self, p: &crate::mdast::Paragraph) {
        self.ensure_newline();
        self.write_inlines(&p.children, None);
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
        self.write_list_impl(l, 0, ListContext::Flow);
    }

    /// Single list renderer shared by document-flow lists and lists inside
    /// definition descriptions. The two contexts intentionally differ in a few
    /// places; see [`ListContext`].
    fn write_list_impl(&mut self, l: &crate::mdast::List, base_indent: usize, ctx: ListContext) {
        let flow = ctx == ListContext::Flow;
        if flow {
            self.ensure_newline();
        }
        // A flow list is "loose" when any item contains more than one block child.
        // Loose lists require blank lines between items and before nested lists.
        // Lists inside definition descriptions are never loose (preserved as-is).
        let is_loose = flow
            && l.children.iter().any(|child| {
                if let Node::ListItem(li) = child {
                    li.children.len() > 1
                } else {
                    false
                }
            });
        let mut num = l.start.unwrap_or(1);
        for (item_index, child) in l.children.iter().enumerate() {
            if let Node::ListItem(li) = child {
                self.output.push_str(&" ".repeat(base_indent));
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
                let item_indent_str = " ".repeat(item_indent);

                for (i, item_child) in li.children.iter().enumerate() {
                    if i > 0 {
                        // Blocks after the first are separated by a blank line.
                        self.write_separator();
                    }
                    match item_child {
                        Node::Paragraph(p) => {
                            if i > 0 {
                                self.output.push_str(&item_indent_str);
                            }
                            // Indent lines after a hard break under the item so they
                            // stay inside it rather than relying on lazy
                            // continuation. Line starts are not escaped, so a
                            // line that looks like a list marker still starts
                            // a nested list.
                            let start = self.output.len();
                            self.write_inlines(&p.children, None);
                            self.reindent_since(start, item_indent);
                            self.write_terminator();
                        }
                        Node::List(nested) if flow => {
                            if i == 0 {
                                // End the marker line; the nested list starts
                                // on the next one.
                                self.write_terminator();
                            }
                            // The recursive call emits the indent per item.
                            self.write_list_impl(nested, item_indent, ctx);
                        }
                        _ => {
                            // Block children (code fences, tables, and in descriptions also
                            // nested lists) have every continuation line re-indented to
                            // `item_indent` so they stay inside the list item.
                            if i > 0 {
                                self.output.push_str(&item_indent_str);
                            } else if !flow {
                                // In descriptions the block always starts on its own line.
                                self.write_terminator();
                                self.output.push_str(&item_indent_str);
                            }
                            self.write_reindented_block(item_child, item_indent);
                            self.close_opaque_block();
                        }
                    }
                }
                if li.children.is_empty() {
                    self.write_terminator();
                }
                // Every item child ends with its own terminator. Items of a loose
                // list are separated by a blank line; lists inside definition
                // descriptions are never loose.
                if is_loose && item_index + 1 < l.children.len() {
                    self.write_separator();
                }
            }
        }
        if flow {
            self.at_line_start = true;
        }
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
                self.write_inlines(&c.children, None);
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
        let mut first_group = true;
        while i < dl.children.len() {
            if let Node::DefinitionTerm(dt) = &dl.children[i] {
                // Term groups are separated by a blank line.
                if !first_group {
                    self.write_separator();
                }
                first_group = false;

                self.write_inlines(&dt.children, None);
                self.write_terminator();

                // Write description(s)
                i += 1;
                let mut after_block_description = false;
                while i < dl.children.len() {
                    if let Node::DefinitionDescription(dd) = &dl.children[i] {
                        // A description that follows a multi-block one is
                        // separated from it by a blank line.
                        if after_block_description {
                            self.write_separator();
                        }
                        after_block_description = self.write_definition_description(dd);
                        i += 1;
                    } else {
                        break;
                    }
                }
            } else {
                i += 1;
            }
        }

        self.at_line_start = true;
    }

    /// Write one `:   ` description, ending with its terminator. Returns
    /// whether it was written in the multi-block form.
    fn write_definition_description(&mut self, dd: &crate::mdast::DefinitionDescription) -> bool {
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

        self.output.push_str(":   ");
        if !has_block_elements {
            // Simple inline content
            for child in &dd.children {
                match child {
                    Node::Paragraph(p) => self.write_inlines(&p.children, None),
                    _ => self.write_node(child),
                }
            }
            // Paragraph text needs its terminator; a raw HTML child may
            // already end with one.
            self.close_opaque_block();
            return false;
        }

        // Pandoc definition lists require a blank line before nested blocks,
        // and all content after the first block must be indented by 4 spaces.
        for (j, child) in dd.children.iter().enumerate() {
            if j > 0 {
                self.write_separator();
            }
            match child {
                Node::Paragraph(p) => {
                    if j > 0 {
                        self.output.push_str("    ");
                    }
                    self.write_inlines(&p.children, None);
                    self.write_terminator();
                }
                Node::List(l) => {
                    // The list indents each of its items by 4 spaces itself.
                    self.write_list_impl(l, 4, ListContext::Description);
                    self.close_opaque_block();
                }
                _ => {
                    if j > 0 {
                        self.output.push_str("    ");
                    }
                    self.write_reindented_block(child, 4);
                    self.close_opaque_block();
                }
            }
        }
        true
    }

    /// Indent every non-empty line after the first of `self.output[start..]`
    /// by `indent` spaces.
    fn reindent_since(&mut self, start: usize, indent: usize) {
        if !self.output[start..].contains('\n') {
            return;
        }
        let indent_str = " ".repeat(indent);
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

    /// Render `node`, then re-indent every line after the first by `indent`
    /// spaces. Used to keep multi-line block children (code fences, nested
    /// lists, tables, ...) properly indented when nested inside a definition
    /// description or a list item. The caller is responsible for indenting
    /// the first line before calling this.
    fn write_reindented_block(&mut self, node: &Node, indent: usize) {
        let start = self.output.len();
        self.write_node(node);
        self.reindent_since(start, indent);
    }
}

/// Where a list is being rendered. The contexts have diverged historically and
/// their differing output is preserved on purpose for now.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ListContext {
    /// Top level or inside another list item: supports loose lists, recurses
    /// into nested lists directly, and always terminates items with a newline.
    Flow,
    /// Inside a definition description: never loose, nested lists are rendered
    /// as ordinary re-indented blocks, and no extra newline follows an item
    /// that already ends with one.
    Description,
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
