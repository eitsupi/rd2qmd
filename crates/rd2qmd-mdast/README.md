# mdast-rd2qmd

mdast types and Quarto Markdown writer for rd2qmd.

## Overview

This crate provides mdast (Markdown Abstract Syntax Tree) types and a writer that outputs Quarto Markdown. It is designed specifically for the rd2qmd project and does not aim to be a complete implementation of either mdast or [quarto-markdown](https://github.com/quarto-dev/quarto-markdown).

**Note**: If you need a general-purpose mdast implementation, consider using [markdown-rs](https://github.com/wooorm/markdown-rs) instead.

## Scope

This crate implements:

- A **subset** of [mdast](https://github.com/syntax-tree/mdast) node types needed for Rd to Markdown conversion
- A writer that outputs Quarto/Pandoc-compatible Markdown

It does **not** implement:

- Full mdast specification (e.g., footnotes, reference-style links)
- Full Quarto Markdown features (e.g., callouts, tabsets, cross-references)
- mdast parsing (only serialization to Markdown)

## mdast Compatibility

### Supported Nodes

| Node Type | Status |
|-----------|--------|
| `Root`, `Paragraph`, `Heading`, `ThematicBreak` | Supported |
| `Blockquote`, `List`, `ListItem` | Supported |
| `Code`, `InlineCode` | Supported |
| `Text`, `Emphasis`, `Strong`, `Break` | Supported |
| `Link`, `Image` | Supported |
| `Table`, `TableRow`, `TableCell` | Supported (GFM extension) |
| `Html` | Supported |

### Not Implemented

- `Definition`, `FootnoteDefinition`, `FootnoteReference`
- `Yaml`, `Toml` (frontmatter handled via `WriterOptions`)
- `Delete` (strikethrough)
- `LinkReference`, `ImageReference`

### Extensions

| Node Type | Description | Output |
|-----------|-------------|--------|
| `DefinitionList`, `DefinitionTerm`, `DefinitionDescription` | Definition lists | Pandoc `:   ` syntax |
| `Math`, `InlineMath` | LaTeX math | `$$...$$` / `$...$` |

## Usage

```rust
use mdast_rd2qmd::{Node, Root, mdast_to_qmd, WriterOptions};

let doc = Root::new(vec![
    Node::heading(1, vec![Node::text("Hello World")]),
    Node::paragraph(vec![
        Node::text("This is "),
        Node::emphasis(vec![Node::text("emphasized")]),
        Node::text(" text."),
    ]),
    Node::code(Some("r".to_string()), "print('Hello')"),
]);

let qmd = mdast_to_qmd(&doc, &WriterOptions::default());
```

Output:

````markdown
# Hello World

This is *emphasized* text.

```r
print('Hello')
```
````

## Writer Options

```rust
use mdast_rd2qmd::{WriterOptions, Frontmatter};

let options = WriterOptions {
    frontmatter: Some(Frontmatter {
        title: Some("Document Title".to_string()),
        pagetitle: Some("Page Title — Section".to_string()),
        format: Some("html".to_string()),
    }),
    // Use {r} for executable R code blocks (requires meta: "executable")
    quarto_code_blocks: true,
};
```

## License

MIT
