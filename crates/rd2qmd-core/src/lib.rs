//! rd2qmd-core: Core library for converting Rd files to Quarto Markdown
//!
//! This crate provides:
//! - Rd file parsing (via rd-parser crate)
//! - Rd AST to mdast conversion
//! - mdast to Quarto Markdown output (via rd2qmd-mdast crate)
//! - Single-file conversion function
//!
//! # API Guide
//!
//! This crate offers three levels of API for different use cases:
//!
//! ## High-level: [`RdConverter`] builder (recommended for most users)
//!
//! Fluent builder API for converting Rd content strings to Quarto Markdown.
//! Handles parsing, conversion, and output in one step.
//!
//! ```
//! use rd2qmd_core::RdConverter;
//!
//! let qmd = RdConverter::new(r#"\name{foo}\title{Foo}\description{A function.}"#)
//!     .frontmatter(true)
//!     .pagetitle(true)
//!     .convert()
//!     .unwrap();
//! ```
//!
//! ## Mid-level: [`convert_rd_content`] function
//!
//! Function-style API when you have a pre-configured [`RdConvertOptions`] struct.
//! Useful when options are loaded from configuration files.
//!
//! ```
//! use rd2qmd_core::{convert_rd_content, RdConvertOptions};
//!
//! let options = RdConvertOptions::default();
//! let qmd = convert_rd_content(r#"\name{foo}\title{Foo}\description{A function.}"#, &options).unwrap();
//! ```
//!
//! [`convert_rd_document`] is the same pipeline for callers that already have
//! a parsed [`RdDocument`] (e.g. from an [`RdAstEnvelope`]); it cannot fail
//! with a parse error.
//!
//! ## Low-level: [`rd_to_mdast`] / [`rd_to_mdast_with_options`]
//!
//! For advanced use cases requiring direct access to the mdast intermediate representation.
//! Use this when you need to manipulate the AST before rendering, or integrate with
//! other markdown processing pipelines.
//!
//! ```
//! use rd2qmd_core::{parse, rd_to_mdast, mdast_to_qmd, WriterOptions};
//!
//! let doc = parse(r#"\name{foo}\title{Foo}\description{A function.}"#).unwrap();
//! let mdast = rd_to_mdast(&doc);
//! // ... manipulate mdast if needed ...
//! let qmd = mdast_to_qmd(&mdast, &WriterOptions::default());
//! ```
//!
//! # Features
//!
//! - `lifecycle`: Enable lifecycle stage extraction from Rd documents
//! - `roxygen`: Enable source file extraction from roxygen2 comments
//!   and roxygen2 markdown code block handling

pub mod ast_io;
pub mod convert;

#[cfg(feature = "roxygen")]
pub mod roxygen_code_block;

use std::collections::HashMap;

// Re-export rd-parser types
pub use rd_parser::{RdDocument, RdNode, RdSection, SectionTag, parse};

pub use ast_io::{AST_FORMAT_VERSION, AstIoError, RdAstEnvelope};

// ============================================================================
// Error types
// ============================================================================

/// Error type for Rd to Markdown conversion
///
/// This provides a stable error interface that doesn't expose internal
/// parser implementation details.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    /// Parse error occurred while processing Rd content
    #[error("Parse error: {0}")]
    Parse(String),
}

#[cfg(feature = "roxygen")]
pub use rd_parser::parse_roxygen_comments;

// Re-export rd2qmd-mdast types
pub use rd2qmd_mdast::{Frontmatter, RdMetadata, WriterOptions, mdast_to_qmd};

pub use convert::{ArgumentsFormat, RdToMdastOptions, rd_to_mdast, rd_to_mdast_with_options};

// ============================================================================
// Option structs for single-file conversion
// ============================================================================

/// Frontmatter output options
#[derive(Debug, Clone, Default)]
pub struct FrontmatterOptions {
    /// Output YAML frontmatter
    pub enabled: bool,
    /// Output pkgdown-style pagetitle (`<title> — <name>`)
    pub pagetitle: bool,
}

/// Code block execution options
#[derive(Debug, Clone)]
pub struct CodeExecutionOptions {
    /// Use Quarto {r} notation for executable code blocks
    pub quarto_code_blocks: bool,
    /// Make \dontrun{} code executable (default: false)
    pub exec_dontrun: bool,
    /// Make \donttest{} code executable (default: true)
    pub exec_donttest: bool,
}

impl Default for CodeExecutionOptions {
    fn default() -> Self {
        Self {
            quarto_code_blocks: true,
            exec_dontrun: false,
            exec_donttest: true,
        }
    }
}

/// Link resolution options
///
/// Rd links come in two classes, each with its own resolution chain:
///
/// - Qualified links (`\link[pkg]{topic}`): `package_urls[pkg]` template,
///   then the `external_link_url` template, then inline code.
/// - Unqualified links (`\link{topic}`): `alias_map` lookup rendered with the
///   `internal_link_url` template, then the `unqualified_link_url` template,
///   then inline code.
#[derive(Debug, Clone, Default)]
pub struct LinkOptions {
    /// URL template for internal links resolved via `alias_map`.
    /// Use {file} for the alias-resolved file basename and {topic} for the
    /// link topic (e.g. `{file}.qmd`).
    /// If None, alias-resolved links become inline code.
    pub internal_link_url: Option<String>,
    /// URL template for unqualified links when alias lookup fails.
    /// Use {topic} as placeholder.
    pub unqualified_link_url: Option<String>,
    /// URL template for qualified links whose package is not in `package_urls`.
    /// Use {package} and {topic} as placeholders.
    pub external_link_url: Option<String>,
    /// Alias to filename map for internal link resolution
    pub alias_map: Option<HashMap<String, String>>,
    /// Package URL map: package name -> full URL template with a {topic}
    /// placeholder (e.g. `https://dplyr.tidyverse.org/reference/{topic}.html`)
    pub package_urls: Option<HashMap<String, String>>,
}

/// Options for single-file Rd to QMD conversion
#[derive(Debug, Clone, Default)]
pub struct RdConvertOptions {
    /// Frontmatter output options
    pub frontmatter: FrontmatterOptions,
    /// Code block execution options
    pub code: CodeExecutionOptions,
    /// Link resolution options
    pub links: LinkOptions,
    /// Arguments section table format
    pub arguments_format: ArgumentsFormat,
    /// Include \if{html}{...} content in the output (default: false)
    pub include_html_output: bool,
    /// Prefer the ASCII representation of `\eqn`/`\deqn` equations over
    /// LaTeX math when one is present (default: false)
    pub prefer_ascii_math: bool,
}

// ============================================================================
// Utility functions
// ============================================================================

/// Extract plain text from Rd nodes
///
/// This function recursively extracts text content from Rd nodes,
/// handling common inline markup like `\code{}`, `\emph{}`, and `\strong{}`.
///
/// # Example
///
/// ```
/// use rd2qmd_core::{parse, extract_text, SectionTag};
///
/// let doc = parse(r#"\name{my_func}\title{My Function}"#).unwrap();
/// if let Some(title) = doc.get_section(&SectionTag::Title) {
///     let text = extract_text(&title.content);
///     assert_eq!(text, "My Function");
/// }
/// ```
pub fn extract_text(nodes: &[RdNode]) -> String {
    let mut result = String::new();
    for node in nodes {
        match node {
            RdNode::Text(s) => result.push_str(s),
            RdNode::Code(children) | RdNode::Emph(children) | RdNode::Strong(children) => {
                result.push_str(&extract_text(children));
            }
            _ => {}
        }
    }
    result.trim().to_string()
}

/// Extract Rd metadata (lifecycle, aliases, keywords, concepts, source_files) from a document
///
/// The `source_files` parameter should be extracted from roxygen2 comments using
/// `rd_parser::parse_roxygen_comments()` (requires the `roxygen` feature).
///
/// # Example
///
/// ```
/// use rd2qmd_core::{parse, extract_rd_metadata};
///
/// let content = r#"\name{foo}\alias{foo}\alias{bar}\keyword{internal}"#;
/// let doc = parse(content).unwrap();
/// let metadata = extract_rd_metadata(&doc, vec![]);
/// assert_eq!(metadata.aliases, vec!["bar", "foo"]);
/// assert_eq!(metadata.keywords, vec!["internal"]);
/// ```
#[cfg(feature = "lifecycle")]
pub fn extract_rd_metadata(doc: &RdDocument, source_files: Vec<String>) -> RdMetadata {
    // Extract lifecycle
    let lifecycle = doc.lifecycle().map(|l| l.as_str().to_string());

    // Extract aliases
    let mut aliases: Vec<String> = doc
        .get_sections(&SectionTag::Alias)
        .iter()
        .map(|s| extract_text(&s.content))
        .filter(|s| !s.is_empty())
        .collect();
    aliases.sort();
    aliases.dedup();

    // Extract keywords
    let mut keywords: Vec<String> = doc
        .get_sections(&SectionTag::Keyword)
        .iter()
        .map(|s| extract_text(&s.content))
        .filter(|s| !s.is_empty())
        .collect();
    keywords.sort();
    keywords.dedup();

    // Extract concepts
    let mut concepts: Vec<String> = doc
        .get_sections(&SectionTag::Concept)
        .iter()
        .map(|s| extract_text(&s.content))
        .filter(|s| !s.is_empty())
        .collect();
    concepts.sort();
    concepts.dedup();

    RdMetadata {
        lifecycle,
        aliases,
        keywords,
        concepts,
        source_files,
    }
}

/// Extract Rd metadata without lifecycle information
///
/// Use this when the `lifecycle` feature is not enabled.
#[cfg(not(feature = "lifecycle"))]
pub fn extract_rd_metadata(doc: &RdDocument, source_files: Vec<String>) -> RdMetadata {
    // Extract aliases
    let mut aliases: Vec<String> = doc
        .get_sections(&SectionTag::Alias)
        .iter()
        .map(|s| extract_text(&s.content))
        .filter(|s| !s.is_empty())
        .collect();
    aliases.sort();
    aliases.dedup();

    // Extract keywords
    let mut keywords: Vec<String> = doc
        .get_sections(&SectionTag::Keyword)
        .iter()
        .map(|s| extract_text(&s.content))
        .filter(|s| !s.is_empty())
        .collect();
    keywords.sort();
    keywords.dedup();

    // Extract concepts
    let mut concepts: Vec<String> = doc
        .get_sections(&SectionTag::Concept)
        .iter()
        .map(|s| extract_text(&s.content))
        .filter(|s| !s.is_empty())
        .collect();
    concepts.sort();
    concepts.dedup();

    RdMetadata {
        lifecycle: None,
        aliases,
        keywords,
        concepts,
        source_files,
    }
}

// ============================================================================
// Single-file Converter Builder
// ============================================================================

/// Builder for single-file Rd to QMD conversion
///
/// This provides a fluent API for converting individual Rd files to Quarto Markdown.
///
/// # Example
///
/// ```
/// use rd2qmd_core::RdConverter;
///
/// let rd_content = r#"
/// \name{hello}
/// \title{Hello World}
/// \description{A simple function.}
/// "#;
///
/// // Basic conversion with defaults
/// let qmd = RdConverter::new(rd_content)
///     .convert()
///     .unwrap();
///
/// // Conversion with custom options
/// let qmd = RdConverter::new(rd_content)
///     .internal_link_url("{file}.md")
///     .frontmatter(true)
///     .pagetitle(true)
///     .quarto_code_blocks(false)
///     .convert()
///     .unwrap();
///
/// assert!(qmd.contains("Hello World"));
/// ```
pub struct RdConverter {
    content: String,
    options: RdConvertOptions,
}

impl RdConverter {
    /// Create a new converter with default options
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            options: RdConvertOptions::default(),
        }
    }

    /// Set the URL template for internal links resolved via the alias map
    /// (e.g. `{file}.qmd`; placeholders: `{file}`, `{topic}`)
    pub fn internal_link_url(mut self, template: impl Into<String>) -> Self {
        self.options.links.internal_link_url = Some(template.into());
        self
    }

    /// Enable or disable YAML frontmatter (default: false)
    pub fn frontmatter(mut self, enabled: bool) -> Self {
        self.options.frontmatter.enabled = enabled;
        self
    }

    /// Enable or disable pkgdown-style pagetitle (default: false)
    pub fn pagetitle(mut self, enabled: bool) -> Self {
        self.options.frontmatter.pagetitle = enabled;
        self
    }

    /// Enable or disable Quarto {r} code blocks (default: true)
    pub fn quarto_code_blocks(mut self, enabled: bool) -> Self {
        self.options.code.quarto_code_blocks = enabled;
        self
    }

    /// Set whether \dontrun{} code is executable (default: false)
    pub fn exec_dontrun(mut self, enabled: bool) -> Self {
        self.options.code.exec_dontrun = enabled;
        self
    }

    /// Set whether \donttest{} code is executable (default: true)
    pub fn exec_donttest(mut self, enabled: bool) -> Self {
        self.options.code.exec_donttest = enabled;
        self
    }

    /// Set the URL template for unqualified links (`\link{topic}`) when alias
    /// lookup fails (placeholder: `{topic}`)
    pub fn unqualified_link_url(mut self, template: impl Into<String>) -> Self {
        self.options.links.unqualified_link_url = Some(template.into());
        self
    }

    /// Set the URL template for qualified links (`\link[pkg]{topic}`) whose
    /// package is not in `package_urls` (placeholders: `{package}`, `{topic}`)
    pub fn external_link_url(mut self, template: impl Into<String>) -> Self {
        self.options.links.external_link_url = Some(template.into());
        self
    }

    /// Set the alias map for internal link resolution
    pub fn alias_map(mut self, map: HashMap<String, String>) -> Self {
        self.options.links.alias_map = Some(map);
        self
    }

    /// Set the package URL map (package name -> full URL template with a
    /// `{topic}` placeholder)
    pub fn package_urls(mut self, urls: HashMap<String, String>) -> Self {
        self.options.links.package_urls = Some(urls);
        self
    }

    /// Set the arguments section format
    pub fn arguments_format(mut self, format: ArgumentsFormat) -> Self {
        self.options.arguments_format = format;
        self
    }

    /// Include \if{html}{...} blocks in the output (default: false)
    pub fn include_html_output(mut self, enabled: bool) -> Self {
        self.options.include_html_output = enabled;
        self
    }

    /// Prefer the ASCII representation of `\eqn`/`\deqn` equations over
    /// LaTeX math when one is present (default: false)
    pub fn prefer_ascii_math(mut self, enabled: bool) -> Self {
        self.options.prefer_ascii_math = enabled;
        self
    }

    /// Set all options at once
    pub fn with_options(mut self, options: RdConvertOptions) -> Self {
        self.options = options;
        self
    }

    /// Execute the conversion
    pub fn convert(self) -> Result<String, ConvertError> {
        convert_rd_content(&self.content, &self.options)
    }
}

/// Convert Rd content to Quarto Markdown
///
/// This is the main entry point for single-file conversion. It parses the Rd content,
/// converts it to mdast, and outputs Quarto Markdown with optional frontmatter.
///
/// For a more flexible API, consider using [`RdConverter`] builder.
///
/// # Example
///
/// ```
/// use rd2qmd_core::{convert_rd_content, RdConvertOptions, FrontmatterOptions, CodeExecutionOptions, LinkOptions};
///
/// let rd_content = r#"
/// \name{hello}
/// \title{Hello World}
/// \description{A simple function.}
/// "#;
///
/// let options = RdConvertOptions {
///     frontmatter: FrontmatterOptions { enabled: true, pagetitle: true },
///     code: CodeExecutionOptions::default(),
///     links: LinkOptions { internal_link_url: Some("{file}.qmd".to_string()), ..Default::default() },
///     ..Default::default()
/// };
///
/// let qmd = convert_rd_content(rd_content, &options).unwrap();
/// assert!(qmd.contains("title:"));
/// assert!(qmd.contains("Hello World"));
/// ```
pub fn convert_rd_content(
    content: &str,
    options: &RdConvertOptions,
) -> Result<String, ConvertError> {
    let doc = parse(content).map_err(|e| ConvertError::Parse(e.to_string()))?;

    #[cfg(feature = "roxygen")]
    let source_files = rd_parser::parse_roxygen_comments(content).source_files;
    #[cfg(not(feature = "roxygen"))]
    let source_files = vec![];

    Ok(convert_rd_document(&doc, source_files, options))
}

/// Convert an already-parsed Rd document to Quarto Markdown
///
/// This is the same conversion pipeline as [`convert_rd_content`], but takes
/// an already-parsed [`RdDocument`] (e.g. deserialized from an
/// [`RdAstEnvelope`]) instead of raw Rd content, so it cannot fail with a
/// parse error.
///
/// `source_files` is the roxygen2-derived list of R source files (see
/// `rd_parser::parse_roxygen_comments`), since that information is extracted
/// from raw Rd text and is not part of the AST itself.
///
/// # Example
///
/// ```
/// use rd2qmd_core::{convert_rd_document, parse, RdConvertOptions};
///
/// let doc = parse(r#"\name{foo}\title{Foo}\description{A function.}"#).unwrap();
/// let qmd = convert_rd_document(&doc, vec![], &RdConvertOptions::default());
/// assert!(qmd.contains("Foo"));
/// ```
pub fn convert_rd_document(
    doc: &RdDocument,
    source_files: Vec<String>,
    options: &RdConvertOptions,
) -> String {
    // Build converter options
    let converter_options = RdToMdastOptions {
        internal_link_url: options.links.internal_link_url.clone(),
        alias_map: options.links.alias_map.clone(),
        unqualified_link_url: options.links.unqualified_link_url.clone(),
        package_urls: options.links.package_urls.clone(),
        external_link_url: options.links.external_link_url.clone(),
        exec_dontrun: options.code.exec_dontrun,
        exec_donttest: options.code.exec_donttest,
        quarto_code_blocks: options.code.quarto_code_blocks,
        arguments_format: options.arguments_format.clone(),
        include_html_output: options.include_html_output,
        prefer_ascii_math: options.prefer_ascii_math,
    };

    // Convert to mdast
    let mdast = rd_to_mdast_with_options(doc, &converter_options);

    // Extract title and name for frontmatter
    let title = doc
        .get_section(&SectionTag::Title)
        .map(|s| extract_text(&s.content));
    let name = doc
        .get_section(&SectionTag::Name)
        .map(|s| extract_text(&s.content));

    // Build pagetitle in pkgdown style: "<title> — <name>"
    let pagetitle = if options.frontmatter.pagetitle {
        match (&title, &name) {
            (Some(t), Some(n)) => Some(format!("{} \u{2014} {}", t, n)),
            _ => None,
        }
    } else {
        None
    };

    // Extract metadata
    let metadata = extract_rd_metadata(doc, source_files);

    // Build writer options
    let writer_options = WriterOptions {
        frontmatter: if options.frontmatter.enabled {
            Some(Frontmatter {
                title,
                pagetitle,
                format: None,
                metadata: Some(metadata),
            })
        } else {
            None
        },
        quarto_code_blocks: options.code.quarto_code_blocks,
    };

    mdast_to_qmd(&mdast, &writer_options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_simple() {
        let nodes = vec![RdNode::Text("Hello World".to_string())];
        assert_eq!(extract_text(&nodes), "Hello World");
    }

    #[test]
    fn test_extract_text_with_markup() {
        let nodes = vec![
            RdNode::Text("Use ".to_string()),
            RdNode::Code(vec![RdNode::Text("foo()".to_string())]),
            RdNode::Text(" for bar".to_string()),
        ];
        assert_eq!(extract_text(&nodes), "Use foo() for bar");
    }

    #[test]
    fn test_extract_text_nested() {
        let nodes = vec![RdNode::Emph(vec![RdNode::Strong(vec![RdNode::Text(
            "nested".to_string(),
        )])])];
        assert_eq!(extract_text(&nodes), "nested");
    }

    #[test]
    fn test_convert_rd_content_basic() {
        let content = r#"\name{test}
\title{Test Function}
\description{A test function.}
"#;
        let options = RdConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: true,
                pagetitle: false,
            },
            ..Default::default()
        };

        let result = convert_rd_content(content, &options).unwrap();
        assert!(result.contains("title: \"Test Function\""));
        assert!(result.contains("# Test Function"));
        assert!(result.contains("A test function."));
    }

    #[test]
    fn test_convert_rd_content_with_pagetitle() {
        let content = r#"\name{foo}
\title{Foo Function}
\description{Does foo.}
"#;
        let options = RdConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: true,
                pagetitle: true,
            },
            ..Default::default()
        };

        let result = convert_rd_content(content, &options).unwrap();
        assert!(result.contains("pagetitle: \"Foo Function — foo\""));
    }

    #[test]
    fn test_convert_rd_content_no_frontmatter() {
        let content = r#"\name{test}
\title{Test}
\description{Description.}
"#;
        let options = RdConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: false,
                pagetitle: false,
            },
            ..Default::default()
        };

        let result = convert_rd_content(content, &options).unwrap();
        assert!(!result.contains("---"));
        assert!(result.contains("# Test"));
    }

    // ========================================================================
    // RdConverter Builder tests
    // ========================================================================

    #[test]
    fn test_rd_converter_basic() {
        let content = r#"\name{hello}
\title{Hello World}
\description{A greeting function.}
"#;
        let result = RdConverter::new(content).convert().unwrap();

        // Default: no frontmatter
        assert!(!result.contains("---"));
        assert!(result.contains("# Hello World"));
        assert!(result.contains("A greeting function."));
    }

    #[test]
    fn test_rd_converter_with_frontmatter() {
        let content = r#"\name{greet}
\title{Greet Function}
\description{Greets the user.}
"#;
        let result = RdConverter::new(content)
            .frontmatter(true)
            .convert()
            .unwrap();

        assert!(result.contains("---"));
        assert!(result.contains("title: \"Greet Function\""));
    }

    #[test]
    fn test_rd_converter_with_pagetitle() {
        let content = r#"\name{myFunc}
\title{My Function}
\description{Does something.}
"#;
        let result = RdConverter::new(content)
            .frontmatter(true)
            .pagetitle(true)
            .convert()
            .unwrap();

        assert!(result.contains("pagetitle: \"My Function — myFunc\""));
    }

    #[test]
    fn test_rd_converter_internal_link_url() {
        let content = r#"\name{foo}
\title{Foo}
\description{Links to \link{bar}.}
"#;
        let mut alias_map = HashMap::new();
        alias_map.insert("bar".to_string(), "bar_file".to_string());

        // Alias hit rendered with the internal link template
        let result = RdConverter::new(content)
            .alias_map(alias_map.clone())
            .internal_link_url("{file}.md")
            .convert()
            .unwrap();
        assert!(result.contains("[`bar`](bar_file.md)"));

        // Without a template, alias hits render as inline code
        let result = RdConverter::new(content)
            .alias_map(alias_map)
            .convert()
            .unwrap();
        assert!(result.contains("`bar`"));
        assert!(!result.contains("bar_file"));
    }

    #[test]
    fn test_rd_converter_quarto_code_blocks() {
        let content = r#"\name{example}
\title{Example}
\examples{
x <- 1
}
"#;
        // With Quarto code blocks (default)
        let result_quarto = RdConverter::new(content)
            .quarto_code_blocks(true)
            .convert()
            .unwrap();
        assert!(result_quarto.contains("```{r}"));

        // Without Quarto code blocks
        let result_plain = RdConverter::new(content)
            .quarto_code_blocks(false)
            .convert()
            .unwrap();
        assert!(result_plain.contains("```r"));
        assert!(!result_plain.contains("```{r}"));
    }

    #[test]
    fn test_rd_converter_exec_dontrun_default() {
        let content = r#"\name{dangerous}
\title{Dangerous}
\examples{
\dontrun{
stop("error")
}
}
"#;
        // Default: dontrun is not executable
        let result = RdConverter::new(content)
            .quarto_code_blocks(true)
            .convert()
            .unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_exec_dontrun_enabled() {
        let content = r#"\name{dangerous}
\title{Dangerous}
\examples{
\dontrun{
stop("error")
}
}
"#;
        // With exec_dontrun: dontrun becomes executable
        let result = RdConverter::new(content)
            .quarto_code_blocks(true)
            .exec_dontrun(true)
            .convert()
            .unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_exec_donttest_default() {
        let content = r#"\name{slow}
\title{Slow}
\examples{
\donttest{
Sys.sleep(10)
}
}
"#;
        // Default: donttest is executable
        let result = RdConverter::new(content)
            .quarto_code_blocks(true)
            .convert()
            .unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_exec_donttest_disabled() {
        let content = r#"\name{slow}
\title{Slow}
\examples{
\donttest{
Sys.sleep(10)
}
}
"#;
        // With exec_donttest(false): donttest is not executable
        let result = RdConverter::new(content)
            .quarto_code_blocks(true)
            .exec_donttest(false)
            .convert()
            .unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_unqualified_link_no_fallback() {
        let content = r#"\name{caller}
\title{Caller}
\description{Uses \link{unknown_func}.}
"#;
        // Without a fallback template: unqualified link becomes inline code
        let result = RdConverter::new(content).convert().unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_unqualified_link_with_fallback() {
        let content = r#"\name{caller}
\title{Caller}
\description{Uses \link{unknown_func}.}
"#;
        // With unqualified_link_url: unqualified link becomes hyperlink
        let result = RdConverter::new(content)
            .unqualified_link_url("https://example.com/{topic}.html")
            .convert()
            .unwrap();
        insta::assert_snapshot!(result);
    }

    const MATH_RD: &str = r#"\name{poisson}
\title{Poisson}
\description{
The density is
\deqn{p(x) = \frac{\lambda^x e^{-\lambda}}{x!}}{p(x) = lambda^x
exp(-lambda) / x!}
for \eqn{x = 0, 1, 2, \ldots}{x = 0, 1, 2, ...}. The mean is
\eqn{\lambda} and \eqn{E(X)}{} equals it.
}
"#;

    #[test]
    fn test_rd_converter_math_default() {
        // Default: LaTeX math output, ASCII representations are ignored
        let result = RdConverter::new(MATH_RD).convert().unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_prefer_ascii_math() {
        // \deqn becomes a plain code block, \eqn becomes inline code with
        // whitespace normalized; equations without a non-blank ASCII
        // representation (one-arg form, empty second arg) stay LaTeX math
        let result = RdConverter::new(MATH_RD)
            .prefer_ascii_math(true)
            .convert()
            .unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_url_autolink() {
        let content = r#"\name{refs}
\title{Refs}
\description{See \url{https://example.com} and \href{https://example.com}{the site}.}
"#;
        // \url is written as an autolink; \href with a distinct text is not
        let result = RdConverter::new(content).convert().unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_external_link_url() {
        let content = r#"\name{wrapper}
\title{Wrapper}
\description{Uses \link[dplyr]{mutate}.}
"#;
        // Qualified link without package_urls: external_link_url applies
        let result = RdConverter::new(content)
            .external_link_url("x-r-help:{package}/{topic}")
            .convert()
            .unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_package_urls_win_over_external_link_url() {
        let content = r#"\name{wrapper}
\title{Wrapper}
\description{Uses \link[dplyr]{mutate}.}
"#;
        let mut package_urls = HashMap::new();
        package_urls.insert(
            "dplyr".to_string(),
            "https://dplyr.tidyverse.org/reference/{topic}.html".to_string(),
        );

        // package_urls takes precedence over external_link_url
        let result = RdConverter::new(content)
            .package_urls(package_urls)
            .external_link_url("x-r-help:{package}/{topic}")
            .convert()
            .unwrap();
        assert!(result.contains("https://dplyr.tidyverse.org/reference/mutate.html"));
        assert!(!result.contains("x-r-help:"));
    }

    #[test]
    fn test_rd_converter_alias_map_wins_over_unqualified_link_url() {
        let content = r#"\name{user}
\title{User}
\description{See \link{helper}.}
"#;
        let mut alias_map = HashMap::new();
        alias_map.insert("helper".to_string(), "utils".to_string());

        // Alias resolution takes precedence over unqualified_link_url
        let result = RdConverter::new(content)
            .alias_map(alias_map)
            .internal_link_url("{file}.qmd")
            .unqualified_link_url("https://example.com/{topic}.html")
            .convert()
            .unwrap();
        assert!(result.contains("[`helper`](utils.qmd)"));
        assert!(!result.contains("example.com"));
    }

    #[test]
    fn test_rd_converter_external_link_url_s4_class() {
        let content = r#"\name{caller}
\title{Caller}
\description{See \linkS4class[methods]{envRefClass} and \linkS4class{MyClass}.}
"#;
        // \linkS4class targets the {classname}-class topic; the qualified link
        // uses external_link_url, the unqualified one uses unqualified_link_url
        let result = RdConverter::new(content)
            .external_link_url("x-r-help:{package}/{topic}")
            .unqualified_link_url("https://example.com/{topic}.html")
            .convert()
            .unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_alias_map() {
        let content = r#"\name{user}
\title{User}
\description{See \link{helper}.}
"#;
        let mut alias_map = HashMap::new();
        alias_map.insert("helper".to_string(), "utils".to_string());

        let result = RdConverter::new(content)
            .internal_link_url("{file}.qmd")
            .alias_map(alias_map)
            .convert()
            .unwrap();

        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_package_urls() {
        let content = r#"\name{wrapper}
\title{Wrapper}
\description{Uses \link[dplyr]{filter}.}
"#;
        let mut package_urls = HashMap::new();
        package_urls.insert(
            "dplyr".to_string(),
            "https://dplyr.tidyverse.org/reference/{topic}.html".to_string(),
        );

        let result = RdConverter::new(content)
            .package_urls(package_urls)
            .convert()
            .unwrap();

        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_arguments_format() {
        let content = r#"\name{args_test}
\title{Arguments Test}
\arguments{
\item{x}{The x value.}
\item{y}{The y value.}
}
"#;
        // Grid table (default)
        let result_grid = RdConverter::new(content)
            .arguments_format(ArgumentsFormat::GridTable)
            .convert()
            .unwrap();
        assert!(result_grid.contains("+---"));

        // Pipe table
        let result_pipe = RdConverter::new(content)
            .arguments_format(ArgumentsFormat::PipeTable)
            .convert()
            .unwrap();
        assert!(result_pipe.contains("| Argument |"));
    }

    #[test]
    fn test_rd_converter_with_options() {
        let content = r#"\name{opts}
\title{Options Test}
\description{Testing with_options.}
"#;
        let options = RdConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: true,
                pagetitle: true,
            },
            code: CodeExecutionOptions {
                quarto_code_blocks: false,
                exec_dontrun: true,
                exec_donttest: false,
            },
            links: LinkOptions {
                internal_link_url: Some("{file}.md".to_string()),
                unqualified_link_url: Some("https://fallback.com/{topic}".to_string()),
                external_link_url: None,
                alias_map: None,
                package_urls: None,
            },
            arguments_format: ArgumentsFormat::PipeTable,
            include_html_output: false,
            prefer_ascii_math: false,
        };

        let result = RdConverter::new(content)
            .with_options(options)
            .convert()
            .unwrap();

        assert!(result.contains("pagetitle: \"Options Test — opts\""));
    }

    #[test]
    fn test_rd_converter_chained_methods() {
        let content = r#"\name{chained}
\title{Chained Builder}
\description{Test chaining.}
"#;
        // All methods can be chained
        let result = RdConverter::new(content)
            .internal_link_url("{file}.qmd")
            .frontmatter(true)
            .pagetitle(true)
            .quarto_code_blocks(true)
            .exec_dontrun(false)
            .exec_donttest(true)
            .arguments_format(ArgumentsFormat::GridTable)
            .convert()
            .unwrap();

        assert!(result.contains("title: \"Chained Builder\""));
        assert!(result.contains("pagetitle:"));
    }

    #[test]
    fn test_rd_converter_parse_error() {
        // Invalid Rd content with unclosed brace
        let content = r#"\name{broken"#;
        let result = RdConverter::new(content).convert();

        assert!(result.is_err());
    }

    #[test]
    fn test_convert_rd_document_matches_convert_rd_content() {
        let content = r#"\name{foo}
\title{Foo Function}
\description{Does foo. See \link{bar}.}
"#;
        let mut alias_map = HashMap::new();
        alias_map.insert("bar".to_string(), "bar_file".to_string());

        let options = RdConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: true,
                pagetitle: true,
            },
            links: LinkOptions {
                internal_link_url: Some("{file}.qmd".to_string()),
                alias_map: Some(alias_map),
                ..Default::default()
            },
            ..Default::default()
        };

        let from_content = convert_rd_content(content, &options).unwrap();

        let doc = parse(content).unwrap();
        #[cfg(feature = "roxygen")]
        let source_files = rd_parser::parse_roxygen_comments(content).source_files;
        #[cfg(not(feature = "roxygen"))]
        let source_files = vec![];
        let from_document = convert_rd_document(&doc, source_files, &options);

        assert_eq!(from_content, from_document);
    }
}
