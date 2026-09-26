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

## Description list output

`DescribeFormat` controls `\describe{}` independently of `ArgumentsFormat`, which
controls the top-level `\arguments{}` section. The default is
`DescribeFormat::DefinitionList` for Pandoc-compatible renderers. For CommonMark
or GFM consumers, select ordinary bullet lists:

```rust
use rd2qmd_core::{ArgumentsFormat, DescribeFormat, RdConvertOptions};

let options = RdConvertOptions {
    arguments_format: ArgumentsFormat::List,
    describe_format: DescribeFormat::List,
    ..Default::default()
};
```

Use `DescribeFormat::Headings` to render terms as headings one level below their
enclosing section. Nested descriptions increase the heading level; descriptions
below H6 switch to bullet lists to preserve further nesting.
Inside `ArgumentsFormat::PipeTable`, headings also fall back to lists, which
are flattened to bold terms, bullet markers, and `<br>` separators. Pipe-table
cells cannot preserve block structure; descriptions outside the table are
unaffected.

`RdToMdastOptions` and `rd2qmd_package::PackageConvertOptions` also expose
`describe_format`. It applies recursively, preserving the terms' inline
formatting and the descriptions' paragraphs, code blocks, and nested lists.

## Dependencies

This crate builds on:

- [`rd-ast`](https://crates.io/crates/rd-ast) - the canonical, producer-neutral Rd document representation
- [`rd2qmd-mdast`](https://crates.io/crates/rd2qmd-mdast) - mdast types and Quarto Markdown writer

## Grid table feature

No Cargo features are enabled by default. `ListTable` (the default), `PipeTable`,
and `List` are always available. Enable `grid-table` to use
`ArgumentsFormat::GridTable` and include the optional `tabled` dependency:

```toml
rd2qmd-core = { version = "0.6", features = ["grid-table"] }
```

Migration for 0.6: `GridTable` no longer exists in the enum when the feature is
disabled. Enable the feature for code that references that variant, or remove
those references when targeting the default configuration. `ArgumentsFormat` is
also non-exhaustive: downstream matches must include a wildcard (`_ => ...`),
even with grid support enabled. This preserves compilation when another
dependency enables additional formats through Cargo feature unification.
Cargo features are additive: another dependency can enable grid support for the
same core crate. The CLI always enables it, preserving
`--arguments-format grid-table` and `output.arguments_format = "grid-table"` in configuration files.

## License

MIT
