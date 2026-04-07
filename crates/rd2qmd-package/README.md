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

```rust
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

```rust
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

```rust
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
    println!("Warning: {} used fallback URL ({:?})", pkg, reason);
}
```

## Features

- `external-links` - Enable external package link resolution. Resolves cross-package `\link[pkg]{topic}` references using installed package metadata and pkgdown URL conventions.

## License

MIT
