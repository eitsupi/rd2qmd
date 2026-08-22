//! Block-level Typst writers.

use super::inline::raw_block;
use super::{TypstWriter, indent_continuation};
use crate::mdast::{Align, ArgumentItem, Arguments, Node};
use crate::writer::ArgumentsFormat;

impl TypstWriter<'_> {
    pub(super) fn write_heading(&mut self, h: &crate::mdast::Heading) {
        self.ensure_newline();
        for _ in 0..h.depth {
            self.output.push('=');
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
        self.output.push_str("#line(length: 100%)\n");
        self.at_line_start = true;
    }

    pub(super) fn write_blockquote(&mut self, b: &crate::mdast::Blockquote) {
        self.ensure_newline();
        let body = self.render_isolated(&b.children);
        self.output.push_str("#quote(block: true)[\n");
        self.output.push_str(&indent_continuation(&body, 2));
        self.output.push_str("\n]\n");
        self.at_line_start = true;
    }

    pub(super) fn write_code(&mut self, c: &crate::mdast::Code) {
        self.ensure_newline();
        let value;
        let code = if c.lang.as_deref() == Some("r") && c.meta.as_deref() == Some("hidden") {
            value = format!("#| echo: false\n#| results: hide\n{}", c.value);
            &value
        } else if c.lang.as_deref() == Some("r") && c.meta.as_deref() != Some("executable") {
            value = format!("#| eval: false\n{}", c.value);
            &value
        } else {
            &c.value
        };
        self.output.push_str(&raw_block(code, c.lang.as_deref()));
        self.output.push('\n');
        self.at_line_start = true;
    }

    pub(super) fn write_list(&mut self, l: &crate::mdast::List, base_indent: usize) {
        self.ensure_newline();
        let mut number = l.start.unwrap_or(1);
        for child in &l.children {
            let Node::ListItem(item) = child else {
                continue;
            };
            let marker = if l.ordered {
                let marker = format!("{number}. ");
                number += 1;
                marker
            } else {
                "- ".to_owned()
            };
            let indent = base_indent + marker.len();

            for _ in 0..base_indent {
                self.output.push(' ');
            }
            self.output.push_str(&marker);
            self.at_line_start = false;

            for (i, item_child) in item.children.iter().enumerate() {
                match item_child {
                    Node::Paragraph(p) => {
                        if i > 0 {
                            // A single newline is a soft line break in Typst;
                            // retain the source paragraph boundary explicitly.
                            self.output.push_str("\n\n");
                            for _ in 0..indent {
                                self.output.push(' ');
                            }
                        }
                        for c in &p.children {
                            self.write_node(c);
                        }
                    }
                    Node::List(nested) => {
                        // A blank line here would close the enclosing item, so
                        // the nested list starts on the very next line.
                        self.output.push('\n');
                        self.at_line_start = true;
                        self.write_list(nested, indent);
                        continue;
                    }
                    other => {
                        if i > 0 {
                            self.output.push('\n');
                            for _ in 0..indent {
                                self.output.push(' ');
                            }
                        }
                        let rendered = self.render_isolated(std::slice::from_ref(other));
                        self.output
                            .push_str(&indent_continuation(&rendered, indent));
                    }
                }
            }
            if !self.at_line_start {
                self.output.push('\n');
            }
            self.at_line_start = true;
        }
        self.at_line_start = true;
    }

    /// Rd `\tabular{}` has no header row, so this is a plain grid.
    pub(super) fn write_table(&mut self, t: &crate::mdast::Table) {
        self.ensure_newline();

        let rows: Vec<&crate::mdast::TableRow> = t
            .children
            .iter()
            .filter_map(|node| match node {
                Node::TableRow(row) => Some(row),
                _ => None,
            })
            .collect();
        if rows.is_empty() {
            return;
        }
        let columns = rows.iter().map(|row| row.children.len()).max().unwrap_or(0);

        self.output
            .push_str(&format!("#table(\n  columns: {columns},\n"));
        if t.align.iter().any(Option::is_some) {
            let align: Vec<_> = (0..columns)
                .map(|i| match t.align.get(i).copied().flatten() {
                    Some(Align::Left) => "left",
                    Some(Align::Center) => "center",
                    Some(Align::Right) => "right",
                    None => "auto",
                })
                .collect();
            self.output
                .push_str(&format!("  align: ({},),\n", align.join(", ")));
        }

        for row in rows {
            let mut line = String::from("  ");
            for cell in &row.children {
                let Node::TableCell(cell) = cell else {
                    continue;
                };
                line.push_str(&self.cell_content(&cell.children));
                line.push_str(", ");
            }
            for _ in row.children.len()..columns {
                line.push_str("[], ");
            }
            let line = line.trim_end();
            self.output.push_str(line);
            self.output.push('\n');
        }
        self.output.push_str(")\n");
        self.at_line_start = true;
    }

    pub(super) fn write_definition_list(&mut self, dl: &crate::mdast::DefinitionList) {
        self.ensure_newline();
        let mut entries: Vec<(String, Vec<Node>)> = Vec::new();

        let mut i = 0;
        while i < dl.children.len() {
            let Node::DefinitionTerm(term) = &dl.children[i] else {
                i += 1;
                continue;
            };
            let term = self.cell_content(&term.children);
            let mut description = Vec::new();
            i += 1;
            while let Some(Node::DefinitionDescription(d)) = dl.children.get(i) {
                description.extend(d.children.iter().cloned());
                i += 1;
            }
            entries.push((term, description));
        }

        if entries.is_empty() {
            return;
        }

        self.output.push_str("#terms(\n");
        for (term, description) in entries {
            let description = self.cell_content(&description);
            self.output
                .push_str(&format!("  terms.item({term}, {description}),\n"));
        }
        self.output.push_str(")\n");
        self.at_line_start = true;
    }

    pub(super) fn write_arguments(&mut self, arguments: &Arguments) {
        if arguments.items.is_empty() {
            return;
        }
        self.ensure_newline();
        match self.options.arguments_format {
            // Every table variant of the Markdown writer exists to work
            // around a Markdown table limitation (pipe tables cannot hold
            // block content, list-tables need Quarto). Typst's own table
            // holds arbitrary content, so all three map to one rendering.
            ArgumentsFormat::PipeTable
            | ArgumentsFormat::GridTable
            | ArgumentsFormat::ListTable => self.write_arguments_table(&arguments.items),
            ArgumentsFormat::List => self.write_arguments_terms(&arguments.items),
        }
    }

    fn write_arguments_table(&mut self, items: &[ArgumentItem]) {
        self.output.push_str("#table(\n  columns: 2,\n");
        self.output
            .push_str("  table.header([Argument], [Description]),\n");
        for item in items {
            let name = super::inline::raw_span(item.name.trim());
            let description = self.cell_content(&item.description);
            self.output
                .push_str(&format!("  [{name}], {description},\n"));
        }
        self.output.push_str(")\n");
        self.at_line_start = true;
    }

    fn write_arguments_terms(&mut self, items: &[ArgumentItem]) {
        self.output.push_str("#terms(\n");
        for item in items {
            let name = super::inline::raw_span(item.name.trim());
            let description = self.cell_content(&item.description);
            self.output
                .push_str(&format!("  terms.item[{name}]{description},\n"));
        }
        self.output.push_str(")\n");
        self.at_line_start = true;
    }

    /// Render nodes as a `[..]` content block suitable for a table cell or a
    /// `terms.item` argument, indented to sit inside the surrounding call.
    pub(crate) fn cell_content(&self, nodes: &[Node]) -> String {
        if nodes.is_empty() {
            return "[]".to_owned();
        }
        let rendered = self.render_isolated(nodes);
        if rendered.contains('\n') {
            format!("[\n    {}\n  ]", indent_continuation(&rendered, 4))
        } else {
            format!("[{rendered}]")
        }
    }
}
