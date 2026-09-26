# rd2qmd-package

Package-level operations for converting R documentation to Quarto Markdown.

## Overview

`rd2qmd-package` provides batch conversion of entire R packages (directories of Rd files) to Quarto Markdown. It handles alias index building for correct internal link resolution and supports parallel processing.

This crate is designed to be used as a library by various interfaces (CLI, R package, etc.).

## Key Types

- **`RdPackage`** - Represents an R package's documentation directory. Scans for `.Rd` files and builds an alias index for link resolution.
- **`PackageConverter`** - Builder for converting an entire package with configurable options.
- **`TopicIndex`** / **`generate_topic_index`** - Generates a JSON index of all topics with metadata (name, title, aliases, lifecycle stage).

## Usage

### Basic conversion

```rust,ignore
use rd2qmd_package::{RdPackage, PackageConvertOptions, PackageConverter};
use std::path::{Path, PathBuf};

let package = RdPackage::from_directory(Path::new("man"), false)?;
let options = PackageConvertOptions {
    output_dir: PathBuf::from("docs/reference"),
    output_extension: "qmd".to_string(),
    ..Default::default()
};

let result = PackageConverter::new(&package, options).convert()?;
println!("Converted {} files", result.conversion.success_count);
```

### Topic index generation

```rust,ignore
use rd2qmd_package::{RdPackage, TopicIndexOptions, generate_topic_index};
use std::path::Path;

let package = RdPackage::from_directory(Path::new("man"), false)?;
let options = TopicIndexOptions {
    output_extension: "qmd".to_string(),
    ..Default::default()
};
let index = generate_topic_index(&package, &options)?;
println!("{}", index.to_json()?);
```

### External link resolution (requires `external-links` feature)

```rust,ignore
use rd2qmd_package::{
    ExternalLinkOptions, PackageConvertOptions, PackageConverter, RdPackage,
};
use std::path::{Path, PathBuf};

let package = RdPackage::from_directory(Path::new("man"), false)?;
let options = PackageConvertOptions {
    output_dir: PathBuf::from("docs/reference"),
    output_extension: "qmd".to_string(),
    ..Default::default()
};

let result = PackageConverter::new(&package, options)
    .with_external_links(ExternalLinkOptions {
        lib_paths: vec![PathBuf::from("/usr/local/lib/R/site-library")],
        ..Default::default()
    })
    .convert()?;

for (pkg, reason) in &result.fallbacks {
    println!("Warning: {} could not be resolved ({:?})", pkg, reason);
}
```

## Features

- `external-links` - Enable external package link resolution. Resolves cross-package `\link[pkg]{topic}` references using installed package metadata and pkgdown URL conventions.

## Grid table feature

No Cargo features are enabled by default. `ListTable` (the default), `PipeTable`,
and `List` are always available. Enable `grid-table` to use
`ArgumentsFormat::GridTable` and include the optional `tabled` dependency:

```toml
rd2qmd-package = { version = "0.6", features = ["grid-table"] }
```

This feature forwards to `rd2qmd-core/grid-table`; the Arguments format is set
through `PackageConvertOptions::arguments_format`.

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
