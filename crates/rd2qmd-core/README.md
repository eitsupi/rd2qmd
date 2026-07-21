# rd2qmd-core

Core library for converting Rd files to Quarto Markdown.

## Overview

`rd2qmd-core` converts an already-parsed `rd_ast::RdDocument` to Quarto Markdown (QMD). It handles AST transformation and Markdown output generation; parsing Rd source text is the responsibility of the `rd2qmd-source` crate (or any other `rd_ast::RdDocument` producer).

This crate is designed to be used as a library by higher-level tools (CLI, R package, etc.).

## API Levels

This crate offers two levels of API for different use cases:

### Mid-level: `convert_rd_document` function

The main entry point for single-document conversion, given a pre-configured `RdConvertOptions` struct.

```rust
use rd2qmd_core::{convert_rd_document, RdConvertOptions};

let doc = rd2qmd_source::parse(r#"\name{foo}\title{Foo}\description{A function.}"#)
    .unwrap()
    .document()
    .clone();
let options = RdConvertOptions::default();
let qmd = convert_rd_document(&doc, &options);
```

### Low-level: `rd_to_mdast` / `rd_to_mdast_with_options`

For advanced use cases requiring direct access to the mdast intermediate representation.
Use this when you need to manipulate the AST before rendering, or integrate with
other Markdown processing pipelines.

```rust
use rd2qmd_core::{rd_to_mdast, mdast_to_qmd, WriterOptions};

let doc = rd2qmd_source::parse(r#"\name{foo}\title{Foo}\description{A function.}"#)
    .unwrap()
    .document()
    .clone();
let mdast = rd_to_mdast(&doc);
// ... manipulate mdast if needed ...
let qmd = mdast_to_qmd(&mdast, &WriterOptions::default());
```

## Dependencies

This crate builds on:

- [`rd-ast`](https://crates.io/crates/rd-ast) - the canonical, producer-neutral Rd document representation
- [`rd2qmd-mdast`](https://crates.io/crates/rd2qmd-mdast) - mdast types and Quarto Markdown writer

## License

MIT
