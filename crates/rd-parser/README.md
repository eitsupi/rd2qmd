# rd-parser

A Rust parser for R Documentation (Rd) files.

## Overview

`rd-parser` provides a lexer, parser, and AST types for parsing Rd files used in R package documentation.

## Features

- Complete lexer (tokenizer) for Rd syntax
- Recursive descent parser
- Strongly-typed AST with serde support
- Support for all standard Rd tags and sections

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

## Supported Tags

### Section Tags

- Required: `\name`, `\title`, `\description`
- Common: `\alias`, `\usage`, `\arguments`, `\value`, `\details`, `\note`, `\author`, `\references`, `\seealso`, `\examples`, `\keyword`, `\concept`, `\format`, `\source`
- Custom: `\section{Title}{...}`

### Inline Tags

- Formatting: `\code`, `\emph`, `\strong`, `\bold`, `\verb`, `\preformatted`
- Links: `\href`, `\link`, `\url`, `\email`
- Math: `\eqn`, `\deqn`
- Lists: `\itemize`, `\enumerate`, `\describe`
- Tables: `\tabular`
- Special: `\R`, `\dots`, `\ldots`, `\cr`, `\tab`
- Conditionals: `\if`, `\ifelse`, `\Sexpr`
- Example control: `\dontrun`, `\donttest`, `\dontshow`, `\testonly`

## Reference

- [Writing R Extensions: Rd format](https://cran.r-project.org/doc/manuals/r-release/R-exts.html#Rd-format)

## License

MIT
