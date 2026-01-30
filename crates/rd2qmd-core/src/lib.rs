//! rd2qmd-core: Core library for converting Rd files to Quarto Markdown
//!
//! This crate provides:
//! - Rd file parsing (via rd-parser crate)
//! - Rd AST to mdast conversion
//! - mdast to Quarto Markdown output (via rd2qmd-mdast crate)
//! - Single-file conversion function
//!
//! ## Features
//!
//! - `lifecycle`: Enable lifecycle stage extraction from Rd documents
//! - `roxygen`: Enable source file extraction from roxygen2 comments
//!   and roxygen2 markdown code block handling

pub mod convert;

#[cfg(feature = "roxygen")]
pub mod roxygen_code_block;

use std::collections::HashMap;

// Re-export rd-parser types
pub use rd_parser::{ParseError, RdDocument, RdNode, RdSection, SectionTag, parse};

#[cfg(feature = "roxygen")]
pub use rd_parser::parse_roxygen_comments;

// Re-export rd2qmd-mdast types
pub use rd2qmd_mdast::{Frontmatter, RdMetadata, WriterOptions, mdast_to_qmd};

pub use convert::{ArgumentsFormat, ConverterOptions, rd_to_mdast, rd_to_mdast_with_options};

// ============================================================================
// Option structs for single-file conversion
// ============================================================================

/// Frontmatter output options
#[derive(Debug, Clone, Default)]
pub struct FrontmatterOptions {
    /// Output YAML frontmatter
    pub enabled: bool,
    /// Output pkgdown-style pagetitle ("<title> — <name>")
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
#[derive(Debug, Clone, Default)]
pub struct LinkOptions {
    /// Output file extension for internal links (e.g., "qmd", "md")
    pub output_extension: String,
    /// Fallback URL pattern for unresolved links. Use {topic} as placeholder.
    pub unresolved_url: Option<String>,
    /// Alias to filename map for internal link resolution
    pub alias_map: Option<HashMap<String, String>>,
    /// External package URL map: package name -> reference documentation base URL
    pub external_package_urls: Option<HashMap<String, String>>,
}

/// Options for single-file Rd to QMD conversion
#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    /// Frontmatter output options
    pub frontmatter: FrontmatterOptions,
    /// Code block execution options
    pub code: CodeExecutionOptions,
    /// Link resolution options
    pub links: LinkOptions,
    /// Arguments section table format
    pub arguments_format: ArgumentsFormat,
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
///     .output_extension("md")
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
    options: ConvertOptions,
}

impl RdConverter {
    /// Create a new converter with default options
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            options: ConvertOptions::default(),
        }
    }

    /// Set the output file extension for link generation (default: "qmd")
    pub fn output_extension(mut self, ext: impl Into<String>) -> Self {
        self.options.links.output_extension = ext.into();
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

    /// Set the fallback URL for unresolved links
    pub fn unresolved_link_url(mut self, url: impl Into<String>) -> Self {
        self.options.links.unresolved_url = Some(url.into());
        self
    }

    /// Set the alias map for internal link resolution
    pub fn alias_map(mut self, map: HashMap<String, String>) -> Self {
        self.options.links.alias_map = Some(map);
        self
    }

    /// Set the external package URL map
    pub fn external_package_urls(mut self, urls: HashMap<String, String>) -> Self {
        self.options.links.external_package_urls = Some(urls);
        self
    }

    /// Set the arguments section format
    pub fn arguments_format(mut self, format: ArgumentsFormat) -> Self {
        self.options.arguments_format = format;
        self
    }

    /// Set all options at once
    pub fn with_options(mut self, options: ConvertOptions) -> Self {
        self.options = options;
        self
    }

    /// Execute the conversion
    pub fn convert(self) -> Result<String, ParseError> {
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
/// use rd2qmd_core::{convert_rd_content, ConvertOptions, FrontmatterOptions, CodeExecutionOptions, LinkOptions};
///
/// let rd_content = r#"
/// \name{hello}
/// \title{Hello World}
/// \description{A simple function.}
/// "#;
///
/// let options = ConvertOptions {
///     frontmatter: FrontmatterOptions { enabled: true, pagetitle: true },
///     code: CodeExecutionOptions::default(),
///     links: LinkOptions { output_extension: "qmd".to_string(), ..Default::default() },
///     ..Default::default()
/// };
///
/// let qmd = convert_rd_content(rd_content, &options).unwrap();
/// assert!(qmd.contains("title:"));
/// assert!(qmd.contains("Hello World"));
/// ```
pub fn convert_rd_content(content: &str, options: &ConvertOptions) -> Result<String, ParseError> {
    let doc = parse(content)?;

    // Build converter options
    let converter_options = ConverterOptions {
        link_extension: Some(options.links.output_extension.clone()),
        alias_map: options.links.alias_map.clone(),
        unresolved_link_url: options.links.unresolved_url.clone(),
        external_package_urls: options.links.external_package_urls.clone(),
        exec_dontrun: options.code.exec_dontrun,
        exec_donttest: options.code.exec_donttest,
        quarto_code_blocks: options.code.quarto_code_blocks,
        arguments_format: options.arguments_format.clone(),
    };

    // Convert to mdast
    let mdast = rd_to_mdast_with_options(&doc, &converter_options);

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
    #[cfg(feature = "roxygen")]
    let source_files = rd_parser::parse_roxygen_comments(content).source_files;
    #[cfg(not(feature = "roxygen"))]
    let source_files = vec![];

    let metadata = extract_rd_metadata(&doc, source_files);

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

    Ok(mdast_to_qmd(&mdast, &writer_options))
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
        let options = ConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: true,
                pagetitle: false,
            },
            links: LinkOptions {
                output_extension: "qmd".to_string(),
                ..Default::default()
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
        let options = ConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: true,
                pagetitle: true,
            },
            links: LinkOptions {
                output_extension: "qmd".to_string(),
                ..Default::default()
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
        let options = ConvertOptions {
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
    fn test_rd_converter_output_extension() {
        let content = r#"\name{foo}
\title{Foo}
\description{Links to \link{bar}.}
"#;
        // With md extension
        let result = RdConverter::new(content)
            .output_extension("md")
            .convert()
            .unwrap();

        // Link should use .md extension when alias is not resolved
        // (unresolved links become inline code by default)
        assert!(result.contains("`bar`"));
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
    fn test_rd_converter_unresolved_link_no_fallback() {
        let content = r#"\name{caller}
\title{Caller}
\description{Uses \link{unknown_func}.}
"#;
        // Without fallback URL: unresolved link becomes inline code
        let result = RdConverter::new(content).convert().unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_unresolved_link_with_fallback() {
        let content = r#"\name{caller}
\title{Caller}
\description{Uses \link{unknown_func}.}
"#;
        // With fallback URL: unresolved link becomes hyperlink
        let result = RdConverter::new(content)
            .unresolved_link_url("https://example.com/{topic}.html")
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
            .output_extension("qmd")
            .alias_map(alias_map)
            .convert()
            .unwrap();

        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_rd_converter_external_package_urls() {
        let content = r#"\name{wrapper}
\title{Wrapper}
\description{Uses \link[dplyr]{filter}.}
"#;
        let mut external_urls = HashMap::new();
        external_urls.insert(
            "dplyr".to_string(),
            "https://dplyr.tidyverse.org/reference".to_string(),
        );

        let result = RdConverter::new(content)
            .external_package_urls(external_urls)
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
        let options = ConvertOptions {
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
                output_extension: "md".to_string(),
                unresolved_url: Some("https://fallback.com/{topic}".to_string()),
                alias_map: None,
                external_package_urls: None,
            },
            arguments_format: ArgumentsFormat::PipeTable,
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
            .output_extension("qmd")
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
}
