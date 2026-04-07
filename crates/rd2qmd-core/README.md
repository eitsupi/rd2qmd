# rd2qmd-core

Core library for converting Rd files to Quarto Markdown.

## Overview

`rd2qmd-core` provides a complete pipeline for converting individual R documentation (Rd) files to Quarto Markdown (QMD). It handles Rd parsing, AST transformation, and Markdown output generation.

This crate is designed to be used as a library by higher-level tools (CLI, R package, etc.).

## API Levels

This crate offers three levels of API for different use cases:

### High-level: `RdConverter` builder (recommended)

Fluent builder API for converting Rd content to Quarto Markdown in one step.

```rust
use rd2qmd_core::RdConverter;

let qmd = RdConverter::new(r#"\name{foo}\title{Foo}\description{A function.}"#)
    .frontmatter(true)
    .pagetitle(true)
    .quarto_code_blocks(true)
    .convert()
    .unwrap();
```

### Mid-level: `convert_rd_content` function

Function-style API when you have a pre-configured `RdConvertOptions` struct. Useful when options are loaded from configuration files.

```rust
use rd2qmd_core::{convert_rd_content, RdConvertOptions};

let options = RdConvertOptions::default();
let qmd = convert_rd_content(
    r#"\name{foo}\title{Foo}\description{A function.}"#,
    &options,
).unwrap();
```

### Low-level: `rd_to_mdast` / `rd_to_mdast_with_options`

For advanced use cases requiring direct access to the mdast intermediate representation.
Use this when you need to manipulate the AST before rendering, or integrate with
other Markdown processing pipelines.

```rust
use rd2qmd_core::{parse, rd_to_mdast, mdast_to_qmd, WriterOptions};

let doc = parse(r#"\name{foo}\title{Foo}\description{A function.}"#).unwrap();
let mdast = rd_to_mdast(&doc);
// ... manipulate mdast if needed ...
let qmd = mdast_to_qmd(&mdast, &WriterOptions::default());
```

## Features

- `lifecycle` - Enable lifecycle stage extraction from Rd documents
- `roxygen` - Enable source file extraction from roxygen2 comments and roxygen2 markdown code block handling

## Dependencies

This crate builds on:

- [`rd-parser`](https://crates.io/crates/rd-parser) - Rd file parsing
- [`mdast-rd2qmd`](https://crates.io/crates/mdast-rd2qmd) - mdast types and Quarto Markdown writer

## License

MIT
