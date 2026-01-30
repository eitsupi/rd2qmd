# rd-parser

A Rust parser for R Documentation (Rd) files.

## Overview

`rd-parser` provides a lexer, parser, and AST types for parsing Rd files used in R package documentation.

## Features

- Complete lexer (tokenizer) for Rd syntax
- Recursive descent parser
- Strongly-typed AST with serde support
- Support for all standard Rd tags and sections

### Optional Features

- `json` - Enables JSON serialization/deserialization methods on `RdDocument`

## Usage

```rust
use rd_parser::{parse, RdDocument, SectionTag};

let source = r#"
\name{example}
\title{Example Function}
\description{An example function.}
"#;

let doc = parse(source).unwrap();
assert_eq!(doc.sections.len(), 3);

// Access sections
if let Some(name_section) = doc.get_section(&SectionTag::Name) {
    println!("Name section found");
}
```

### JSON Serialization (requires `json` feature)

```rust
use rd_parser::{parse, RdDocument};

let source = r#"\name{example}\title{Example}"#;
let doc = parse(source).unwrap();

// Serialize to JSON
let json = doc.to_json_pretty().unwrap();
println!("{}", json);

// Deserialize from JSON
let restored = RdDocument::from_json(&json).unwrap();
```

## Supported Tags

### Section Tags

- Required: `\name`, `\title`, `\description`
- Common: `\alias`, `\usage`, `\arguments`, `\value`, `\details`, `\note`, `\author`, `\references`, `\seealso`, `\examples`, `\keyword`, `\concept`, `\format`, `\source`
- Custom: `\section{Title}{...}`

### Inline Tags

- Formatting: `\code`, `\emph`, `\strong`, `\bold`, `\verb`, `\preformatted`, `\cite`, `\abbr`
- Links: `\href`, `\link`, `\linkS4class`, `\url`, `\email`, `\doi`
- Math: `\eqn`, `\deqn`
- Lists: `\itemize`, `\enumerate`, `\describe`
- Tables: `\tabular`
- Special: `\R`, `\dots`, `\ldots`, `\cr`, `\tab`
- Conditionals: `\if`, `\ifelse`, `\Sexpr`
- Methods: `\method`, `\S3method`, `\S4method`
- Example control: `\dontrun`, `\donttest`, `\dontshow`, `\testonly`, `\dontdiff`

## Reference

- [Writing R Extensions: Rd format](https://cran.r-project.org/doc/manuals/r-release/R-exts.html#Rd-format)

## License

MIT
