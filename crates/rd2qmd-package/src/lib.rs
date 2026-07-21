//! rd2qmd-package: Package-level operations for Rd to QMD conversion
//!
//! This crate provides functionality for converting entire R packages
//! (directories of Rd files) to Quarto Markdown, including:
//! - Alias index building for correct link resolution
//! - Batch conversion with parallel processing
//!
//! This crate is designed to be used by various interfaces (CLI, R package, etc.)
//!
//! ## Features
//!
//! - `external-links`: Enable external package link resolution (requires network access)

#[cfg(feature = "external-links")]
pub mod external_links;

#[cfg(feature = "external-links")]
pub use external_links::{
    PackageResolveResult, PackageUrlResolver, PackageUrlResolverOptions, collect_external_packages,
};

/// Reason why a package could not be resolved to its pkgdown documentation URL
///
/// This is returned in [`FullConvertResult::fallbacks`] when external link resolution
/// is enabled and a package could not be resolved. Links to such packages fall
/// back to the `external_link_url` template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackReason {
    /// Package is not installed in any of the library paths
    NotInstalled,
    /// Package is installed but no pkgdown site could be found
    NoPkgdownSite,
}

use rayon::prelude::*;
use rd2qmd_core::{
    ArgumentsFormat, Frontmatter, RdAstEnvelope, RdDocument, RdMetadata, RdToMdastOptions,
    WriterOptions, extract_rd_metadata, extract_text, mdast_to_qmd, rd_to_mdast_with_options,
};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Errors that can occur during package operations
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error in {file}: {message}")]
    Parse { file: PathBuf, message: String },

    #[error("Directory not found: {0}")]
    DirectoryNotFound(PathBuf),
}

/// Result type for package operations
pub type Result<T> = std::result::Result<T, PackageError>;

/// Internal error type for single-file conversion
///
/// Used within [`convert_single_file`] to distinguish between
/// files that should be skipped (internal) and actual errors.
#[derive(Debug)]
enum ConvertError {
    /// File has `\keyword{internal}` and should be skipped
    SkipInternal,
    /// Conversion failed with an error message
    Failed(String),
}

/// Input format for a package's documentation files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputFormat {
    /// Parse `.Rd` files (default)
    #[default]
    Rd,
    /// Decode pre-parsed AST JSON envelopes (`.json` files, see [`RdAstEnvelope`])
    AstJson,
}

/// Information about an R package's documentation
#[derive(Debug, Clone)]
pub struct RdPackage {
    /// Root directory containing Rd files
    root: PathBuf,
    /// List of Rd files in the package
    files: Vec<PathBuf>,
    /// Alias index: maps alias names to Rd file basenames (without extension)
    alias_index: HashMap<String, String>,
    /// Format of the files in this package
    format: InputFormat,
}

impl RdPackage {
    /// Load a package from a directory containing Rd files
    ///
    /// This scans the directory for .Rd files and builds an alias index
    /// by parsing each file and extracting \alias{} tags.
    pub fn from_directory(path: &Path, recursive: bool) -> Result<Self> {
        Self::from_directory_with_format(path, recursive, InputFormat::Rd)
    }

    /// Load a package from a directory containing Rd files or AST JSON files
    ///
    /// With [`InputFormat::AstJson`], the directory is scanned for `.json`
    /// files instead of `.Rd`/`.rd` files, and each file is decoded as an
    /// [`RdAstEnvelope`] instead of being parsed as Rd.
    pub fn from_directory_with_format(
        path: &Path,
        recursive: bool,
        format: InputFormat,
    ) -> Result<Self> {
        if !path.is_dir() {
            return Err(PackageError::DirectoryNotFound(path.to_path_buf()));
        }

        let files = collect_files(path, recursive, format)?;
        let alias_index = build_alias_index(&files, format)?;

        Ok(Self {
            root: path.to_path_buf(),
            files,
            alias_index,
            format,
        })
    }

    /// Get the root directory containing Rd files
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the list of Rd files in the package
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Get the alias index (maps alias names to Rd file basenames)
    pub fn alias_index(&self) -> &HashMap<String, String> {
        &self.alias_index
    }

    /// Get the target filename for a given alias
    ///
    /// Returns the Rd file basename (without extension) that contains this alias,
    /// or None if the alias is not found.
    pub fn resolve_alias(&self, alias: &str) -> Option<&str> {
        self.alias_index.get(alias).map(|s| s.as_str())
    }
}

/// Options for package conversion
#[derive(Debug, Clone)]
pub struct PackageConvertOptions {
    /// Output directory for converted files
    pub output_dir: PathBuf,
    /// File extension for output files (e.g., "qmd", "md")
    pub output_extension: String,
    /// Whether to add YAML frontmatter
    pub frontmatter: bool,
    /// Whether to add pagetitle in pkgdown style (`<title> — <name>`)
    pub pagetitle: bool,
    /// Whether to use Quarto {r} code blocks for examples
    pub quarto_code_blocks: bool,
    /// Number of parallel jobs (None = use all CPUs)
    pub parallel_jobs: Option<usize>,
    /// URL template for internal links resolved via the alias index.
    /// Use `{file}` as placeholder for the alias-resolved file basename and
    /// `{topic}` for the link topic.
    /// If None, the template is derived from `output_extension` as
    /// `{file}.<output_extension>`.
    pub internal_link_url: Option<String>,
    /// URL template for unqualified links (`\link{topic}`) when alias lookup fails.
    /// Use `{topic}` as placeholder for the topic name.
    /// Example: `https://rdrr.io/r/base/{topic}.html`
    /// If None, such links become inline code instead of hyperlinks
    pub unqualified_link_url: Option<String>,
    /// Package URL map: package name -> full URL template with a `{topic}`
    /// placeholder. Used for resolving qualified links (`\link[pkg]{topic}`).
    /// Example: `{"dplyr" -> "https://dplyr.tidyverse.org/reference/{topic}.html"}`
    pub package_urls: Option<HashMap<String, String>>,
    /// URL template for qualified links (`\link[pkg]{topic}`) whose package is
    /// not found in `package_urls`.
    /// Use `{package}` and `{topic}` as placeholders.
    /// Example: `https://rdrr.io/pkg/{package}/man/{topic}.html`
    /// If None, such links become inline code instead of hyperlinks
    pub external_link_url: Option<String>,
    /// Make \dontrun{} example code executable (default: false)
    /// Matches pkgdown semantics: \dontrun{} means "never run this code"
    pub exec_dontrun: bool,
    /// Make \donttest{} example code executable (default: true)
    /// Matches pkgdown semantics: \donttest{} means "don't run during testing"
    /// but the code should normally be executable
    pub exec_donttest: bool,
    /// Include topics with \keyword{internal} (default: false)
    /// By default, internal topics are skipped (matching pkgdown behavior).
    /// Set to true to include internal topics in the output.
    pub include_internal: bool,
    /// Include \if{html}{...} content in the output (default: false)
    /// By default, HTML-conditional blocks are excluded because they target
    /// HTML renderers and often contain raw HTML markup that produces noise in
    /// plain Markdown. Set to true when targeting an HTML-capable renderer.
    pub include_html_output: bool,
    /// Prefer the ASCII representation of `\eqn`/`\deqn` equations over
    /// LaTeX math when one is present (default: false)
    pub prefer_ascii_math: bool,
    /// Table format for the Arguments section
    pub arguments_format: ArgumentsFormat,
}

impl Default for PackageConvertOptions {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("."),
            output_extension: "qmd".to_string(),
            frontmatter: true,
            pagetitle: true,
            quarto_code_blocks: true,
            parallel_jobs: None,
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true, // pkgdown-compatible: \donttest{} is executable by default
            include_internal: false, // pkgdown-compatible: skip internal topics by default
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        }
    }
}

/// Result of a package conversion
#[derive(Debug)]
pub struct ConvertResult {
    /// Number of successfully converted files
    pub success_count: usize,
    /// Files that failed to convert, with their errors
    pub failed_files: Vec<(PathBuf, String)>,
    /// Output files that were created
    pub output_files: Vec<PathBuf>,
    /// Files skipped because they have \keyword{internal}
    pub skipped_internal: Vec<PathBuf>,
}

/// Information about a single topic (Rd file) for index generation
#[derive(Debug, Clone, Serialize)]
pub struct TopicInfo {
    /// Topic name (from \name{})
    pub name: String,
    /// Output filename (e.g., "foo.qmd")
    pub file: String,
    /// Topic title (from \title{})
    pub title: String,
    /// Rd metadata (lifecycle, aliases, keywords, concepts, source_files)
    #[serde(flatten)]
    pub metadata: RdMetadata,
}

/// Index of all topics in a package
#[derive(Debug, Clone, Serialize)]
pub struct TopicIndex {
    /// List of topics
    pub topics: Vec<TopicInfo>,
}

impl TopicIndex {
    /// Serialize the index to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| PackageError::Io(std::io::Error::other(e)))
    }
}

/// Options for topic index generation
#[derive(Debug, Clone, Default)]
pub struct TopicIndexOptions {
    /// File extension for output files (e.g., "qmd", "md")
    pub output_extension: String,
    /// Include topics with \keyword{internal} (default: false)
    /// By default, internal topics are excluded from the index.
    pub include_internal: bool,
}

/// Generate a topic index from a package
///
/// This function parses all Rd files in the package and extracts metadata
/// for each topic, including name, title, aliases, and lifecycle stage.
///
/// # Example
///
/// ```ignore
/// let package = RdPackage::from_directory(Path::new("man"), false)?;
/// let options = TopicIndexOptions {
///     output_extension: "qmd".to_string(),
/// };
/// let index = generate_topic_index(&package, &options)?;
/// println!("{}", index.to_json()?);
/// ```
pub fn generate_topic_index(
    package: &RdPackage,
    options: &TopicIndexOptions,
) -> Result<TopicIndex> {
    let mut topics = Vec::new();

    for file in &package.files {
        match extract_topic_info(file, &options.output_extension, package.format) {
            Ok(info) => {
                // Skip internal topics unless include_internal is set
                if !options.include_internal
                    && info.metadata.keywords.contains(&"internal".to_string())
                {
                    continue;
                }
                topics.push(info);
            }
            Err(e) => {
                // Log error but continue processing other files
                eprintln!(
                    "Warning: failed to extract topic info from {}: {}",
                    file.display(),
                    e
                );
            }
        }
    }

    // Sort by name for consistent output
    topics.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(TopicIndex { topics })
}

/// Extract topic information from a single Rd file
fn extract_topic_info(
    file: &Path,
    output_extension: &str,
    format: InputFormat,
) -> Result<TopicInfo> {
    let (doc, source_files) = load_document(file, format)?;

    // Extract name
    let name = doc.name().map(extract_text).unwrap_or_default();

    // Extract title
    let title = doc.title().map(extract_text).unwrap_or_default();

    // Extract metadata using shared function
    let metadata = rd2qmd_core::RdMetadata {
        source_files,
        ..extract_rd_metadata(&doc)
    };

    // Determine output filename
    let basename = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let output_file = format!("{}.{}", basename, output_extension);

    Ok(TopicInfo {
        name,
        file: output_file,
        title,
        metadata,
    })
}

/// Outcome of converting a single file
enum ConvertOutcome {
    /// Successfully converted, contains output path
    Success(PathBuf),
    /// Skipped because the topic has \keyword{internal}
    SkippedInternal(PathBuf),
    /// Failed to convert, contains input path and error message
    Failed(PathBuf, String),
}

/// Convert an entire package to Quarto Markdown
///
/// This function converts all Rd files in the package, using the alias index
/// to resolve internal links correctly.
pub fn convert_package(
    package: &RdPackage,
    options: &PackageConvertOptions,
) -> Result<ConvertResult> {
    // Configure thread pool if specified
    if let Some(n) = options.parallel_jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .ok();
    }

    // Create output directory if needed
    fs::create_dir_all(&options.output_dir)?;

    // Convert files in parallel
    let results: Vec<_> = package
        .files
        .par_iter()
        .map(|file| convert_single_file(file, package, options))
        .collect();

    // Collect results
    let mut success_count = 0;
    let mut failed_files = Vec::new();
    let mut output_files = Vec::new();
    let mut skipped_internal = Vec::new();

    for result in results {
        match result {
            ConvertOutcome::Success(output_path) => {
                success_count += 1;
                output_files.push(output_path);
            }
            ConvertOutcome::SkippedInternal(input_path) => {
                skipped_internal.push(input_path);
            }
            ConvertOutcome::Failed(path, error) => {
                failed_files.push((path, error));
            }
        }
    }

    Ok(ConvertResult {
        success_count,
        failed_files,
        output_files,
        skipped_internal,
    })
}

/// Export all Rd files in a directory as AST JSON envelopes
///
/// Each `.Rd` file is parsed, its roxygen2-derived `source_files` are
/// extracted, and the result is wrapped in an [`RdAstEnvelope`] and written
/// as a `<stem>.json` file, mirroring the input file's relative path under
/// `output_dir`.
///
/// Unlike [`convert_package`], this does not build an alias index (link
/// resolution happens later, at conversion time) and applies no
/// `\keyword{internal}` filtering: every file is exported, since filtering
/// is the responsibility of the final conversion stage.
pub fn export_package_ast(
    input_dir: &Path,
    recursive: bool,
    output_dir: &Path,
    jobs: Option<usize>,
) -> Result<ConvertResult> {
    if !input_dir.is_dir() {
        return Err(PackageError::DirectoryNotFound(input_dir.to_path_buf()));
    }

    // Configure thread pool if specified
    if let Some(n) = jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .ok();
    }

    let files = collect_files(input_dir, recursive, InputFormat::Rd)?;

    // Create output directory if needed
    fs::create_dir_all(output_dir)?;

    // Export files in parallel
    let results: Vec<_> = files
        .par_iter()
        .map(|file| export_single_file(file, input_dir, output_dir))
        .collect();

    // Collect results
    let mut success_count = 0;
    let mut failed_files = Vec::new();
    let mut output_files = Vec::new();

    for result in results {
        match result {
            Ok(output_path) => {
                success_count += 1;
                output_files.push(output_path);
            }
            Err((input_path, message)) => {
                failed_files.push((input_path, message));
            }
        }
    }

    Ok(ConvertResult {
        success_count,
        failed_files,
        output_files,
        skipped_internal: Vec::new(),
    })
}

/// Parse a single Rd file and write it as an AST JSON envelope
fn export_single_file(
    input: &Path,
    root: &Path,
    output_dir: &Path,
) -> std::result::Result<PathBuf, (PathBuf, String)> {
    let export = || -> std::result::Result<PathBuf, String> {
        let content = fs::read_to_string(input).map_err(|e| e.to_string())?;

        let parsed = rd2qmd_source::parse(&content).map_err(|e| format!("Parse error: {e}"))?;
        let doc = parsed.document().clone();
        let source_files = extract_rd_metadata(&doc).source_files;

        let source = input
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        let envelope = RdAstEnvelope::new(doc, source, source_files);
        let json = envelope.to_json_pretty().map_err(|e| e.to_string())?;

        // Determine output path
        let relative = input.strip_prefix(root).unwrap_or(input);
        let output_path = output_dir.join(relative).with_extension("json");

        // Create parent directory if needed
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Write output
        fs::write(&output_path, json).map_err(|e| e.to_string())?;

        Ok(output_path)
    };

    export().map_err(|message| (input.to_path_buf(), message))
}

/// Check if a document has \keyword{internal}
fn has_keyword_internal(doc: &rd2qmd_core::RdDocument) -> bool {
    doc.keywords()
        .any(|keyword| keyword.eq_ignore_ascii_case("internal"))
}

/// Convert a single Rd file
fn convert_single_file(
    input: &Path,
    package: &RdPackage,
    options: &PackageConvertOptions,
) -> ConvertOutcome {
    let convert = || -> std::result::Result<PathBuf, ConvertError> {
        // Read and parse the input file (Rd or AST JSON, depending on package.format)
        let (doc, source_files) = load_document(input, package.format)
            .map_err(|e| ConvertError::Failed(e.to_string()))?;

        // Check for \keyword{internal} - skip unless include_internal is set
        if !options.include_internal && has_keyword_internal(&doc) {
            return Err(ConvertError::SkipInternal);
        }

        // Build converter options with alias map. Unless overridden, the
        // internal link template is derived from the output file extension.
        let internal_link_url = options
            .internal_link_url
            .clone()
            .unwrap_or_else(|| format!("{{file}}.{}", options.output_extension));
        let converter_options = RdToMdastOptions {
            include_title_heading: !options.frontmatter,
            internal_link_url: Some(internal_link_url),
            alias_map: Some(package.alias_index.clone()),
            unqualified_link_url: options.unqualified_link_url.clone(),
            package_urls: options.package_urls.clone(),
            external_link_url: options.external_link_url.clone(),
            exec_dontrun: options.exec_dontrun,
            exec_donttest: options.exec_donttest,
            quarto_code_blocks: options.quarto_code_blocks,
            arguments_format: options.arguments_format.clone(),
            include_html_output: options.include_html_output,
            prefer_ascii_math: options.prefer_ascii_math,
        };

        // Convert to mdast
        let mdast = rd_to_mdast_with_options(&doc, &converter_options);

        // Extract title and name for frontmatter
        let title = doc.title().map(extract_text);
        let name = doc.name().map(extract_text);

        // Build pagetitle in pkgdown style: "<title> — <name>"
        let pagetitle = if options.pagetitle {
            match (&title, &name) {
                (Some(t), Some(n)) => Some(format!("{} \u{2014} {}", t, n)),
                _ => None,
            }
        } else {
            None
        };

        // Extract Rd metadata, including source files from roxygen2 comments
        let metadata = rd2qmd_core::RdMetadata {
            source_files,
            ..extract_rd_metadata(&doc)
        };

        // Build writer options
        let writer_options = WriterOptions {
            frontmatter: if options.frontmatter {
                Some(Frontmatter {
                    title,
                    pagetitle,
                    format: None,
                    metadata: Some(metadata),
                })
            } else {
                None
            },
            quarto_code_blocks: options.quarto_code_blocks,
        };

        // Convert to QMD string
        let qmd = mdast_to_qmd(&mdast, &writer_options);

        // Determine output path
        let relative = input.strip_prefix(&package.root).unwrap_or(input);
        let output_path = options
            .output_dir
            .join(relative)
            .with_extension(&options.output_extension);

        // Create parent directory if needed
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ConvertError::Failed(e.to_string()))?;
        }

        // Write output
        fs::write(&output_path, qmd).map_err(|e| ConvertError::Failed(e.to_string()))?;

        Ok(output_path)
    };

    match convert() {
        Ok(path) => ConvertOutcome::Success(path),
        Err(ConvertError::SkipInternal) => ConvertOutcome::SkippedInternal(input.to_path_buf()),
        Err(ConvertError::Failed(msg)) => ConvertOutcome::Failed(input.to_path_buf(), msg),
    }
}

/// Collect all documentation files in a directory
///
/// Scans for `.Rd`/`.rd` files with [`InputFormat::Rd`], or `.json` files
/// with [`InputFormat::AstJson`].
fn collect_files(dir: &Path, recursive: bool, format: InputFormat) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let matches = match format {
                InputFormat::Rd => path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("rd")),
                InputFormat::AstJson => path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json")),
            };
            if matches {
                files.push(path);
            }
        } else if path.is_dir() && recursive {
            files.extend(collect_files(&path, recursive, format)?);
        }
    }

    Ok(files)
}

/// Read and parse a single documentation file, returning its AST and the
/// roxygen2-derived list of R source files
///
/// With [`InputFormat::Rd`], the file is read as Rd source and parsed; the
/// source files are extracted from roxygen2 header comments. With
/// [`InputFormat::AstJson`], the file is decoded as an [`RdAstEnvelope`]
/// instead, and its `source_files` field is used directly.
fn load_document(path: &Path, format: InputFormat) -> Result<(RdDocument, Vec<String>)> {
    let content = fs::read_to_string(path)?;

    match format {
        InputFormat::Rd => {
            let parsed = rd2qmd_source::parse(&content).map_err(|e| PackageError::Parse {
                file: path.to_path_buf(),
                message: e.to_string(),
            })?;
            let doc = parsed.document().clone();
            let source_files = extract_rd_metadata(&doc).source_files;
            Ok((doc, source_files))
        }
        InputFormat::AstJson => {
            let envelope = RdAstEnvelope::from_json(&content).map_err(|e| PackageError::Parse {
                file: path.to_path_buf(),
                message: e.to_string(),
            })?;
            Ok((envelope.document, envelope.source_files))
        }
    }
}

/// Build an alias index from a list of documentation files
///
/// Returns a HashMap mapping alias names to file basenames (without extension)
fn build_alias_index(files: &[PathBuf], format: InputFormat) -> Result<HashMap<String, String>> {
    let mut index = HashMap::new();

    for file in files {
        let basename = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let (doc, _source_files) = load_document(file, format)?;

        for alias in doc.aliases() {
            let alias = alias.trim().to_string();
            if !alias.is_empty() {
                index.insert(alias, basename.clone());
            }
        }

        // Also add \name{} as an alias (it's always a valid reference)
        if let Some(name_nodes) = doc.name() {
            let name = extract_text(name_nodes).trim().to_string();
            if !name.is_empty() {
                index.insert(name, basename.clone());
            }
        }
    }

    Ok(index)
}

// ============================================================================
// Package Converter Builder
// ============================================================================

/// Options for external link resolution during package conversion
#[cfg(feature = "external-links")]
#[derive(Debug, Clone, Default)]
pub struct ExternalLinkOptions {
    /// R library paths to search for installed packages
    pub lib_paths: Vec<PathBuf>,
    /// Cache directory for pkgdown.yml files
    pub cache_dir: Option<PathBuf>,
}

/// Result of a package conversion
///
/// This includes both the conversion results and any external link resolution fallbacks.
/// When external link resolution is not used, `fallbacks` will be empty.
#[derive(Debug)]
pub struct FullConvertResult {
    /// Basic conversion result
    pub conversion: ConvertResult,
    /// Packages that could not be resolved during external link resolution
    /// (package name -> reason). Links to these packages fall back to the
    /// `external_link_url` template.
    /// Empty when external link resolution is not enabled or not used.
    pub fallbacks: HashMap<String, FallbackReason>,
}

/// Builder for package conversion
///
/// This provides a fluent API for converting R packages to Quarto Markdown.
///
/// # Example
///
/// ```ignore
/// use rd2qmd_package::{RdPackage, PackageConvertOptions, PackageConverter};
/// use std::path::PathBuf;
///
/// let package = RdPackage::from_directory(Path::new("man"), false)?;
/// let options = PackageConvertOptions {
///     output_dir: PathBuf::from("output"),
///     output_extension: "qmd".to_string(),
///     ..Default::default()
/// };
///
/// // Basic conversion
/// let result = PackageConverter::new(&package, options).convert()?;
/// println!("Converted {} files", result.conversion.success_count);
/// ```
///
/// With external link resolution (requires `external-links` feature):
///
/// ```ignore
/// use rd2qmd_package::{ExternalLinkOptions, PackageConverter};
///
/// let result = PackageConverter::new(&package, options)
///     .with_external_links(ExternalLinkOptions {
///         lib_paths: vec![PathBuf::from("/usr/local/lib/R/site-library")],
///         ..Default::default()
///     })
///     .convert()?;
///
/// for (pkg, reason) in &result.fallbacks {
///     println!("Warning: {} could not be resolved ({:?})", pkg, reason);
/// }
/// ```
pub struct PackageConverter<'a> {
    package: &'a RdPackage,
    options: PackageConvertOptions,
    #[cfg(feature = "external-links")]
    external_opts: Option<ExternalLinkOptions>,
}

impl<'a> PackageConverter<'a> {
    /// Create a new package converter
    pub fn new(package: &'a RdPackage, options: PackageConvertOptions) -> Self {
        Self {
            package,
            options,
            #[cfg(feature = "external-links")]
            external_opts: None,
        }
    }

    /// Enable external link resolution
    ///
    /// When enabled, the converter will:
    /// 1. Collect external package references from `\link[pkg]{topic}` patterns
    /// 2. Resolve package documentation URL templates from installed packages
    ///    or pkgdown sites and merge them into `package_urls` (user-provided
    ///    entries take precedence)
    /// 3. Report packages that could not be resolved in
    ///    [`FullConvertResult::fallbacks`]; such links fall back to the
    ///    `external_link_url` template
    #[cfg(feature = "external-links")]
    pub fn with_external_links(mut self, opts: ExternalLinkOptions) -> Self {
        self.external_opts = Some(opts);
        self
    }

    /// Execute the conversion
    ///
    /// Returns a `FullConvertResult` containing:
    /// - `conversion`: The basic conversion result (success count, failed files, output files)
    /// - `fallbacks`: External package URL resolution fallbacks (empty if external links not used)
    pub fn convert(self) -> Result<FullConvertResult> {
        #[cfg(feature = "external-links")]
        {
            self.convert_with_external_links()
        }

        #[cfg(not(feature = "external-links"))]
        {
            let conversion = convert_package(self.package, &self.options)?;
            Ok(FullConvertResult {
                conversion,
                fallbacks: HashMap::new(),
            })
        }
    }

    #[cfg(feature = "external-links")]
    fn convert_with_external_links(mut self) -> Result<FullConvertResult> {
        let mut fallbacks = HashMap::new();

        // Resolve external package URLs if options are provided
        if let Some(ext_opts) = self.external_opts
            && !ext_opts.lib_paths.is_empty()
        {
            // Collect external package references. Packages already covered
            // by user-provided package_urls need no automatic resolution:
            // resolving them anyway would trigger needless HTTP requests and
            // report false fallbacks for links that resolve just fine.
            let mut external_packages = collect_external_packages(self.package);
            if let Some(user_urls) = &self.options.package_urls {
                external_packages.retain(|pkg| !user_urls.contains_key(pkg));
            }

            if !external_packages.is_empty() {
                // Resolve URLs
                let mut resolver = PackageUrlResolver::new(PackageUrlResolverOptions {
                    lib_paths: ext_opts.lib_paths,
                    cache_dir: ext_opts.cache_dir,
                    enable_http: true,
                });
                let resolve_result = resolver.resolve_packages(&external_packages);

                // Store fallbacks for reporting
                fallbacks = resolve_result.fallbacks;

                // Merge resolved URL templates into options; user-provided
                // package_urls entries take precedence over auto-resolved ones
                let mut urls = resolve_result.urls;
                if let Some(user_urls) = self.options.package_urls.take() {
                    urls.extend(user_urls);
                }
                if !urls.is_empty() {
                    self.options.package_urls = Some(urls);
                }
            }
        }

        // Convert package
        let conversion = convert_package(self.package, &self.options)?;

        Ok(FullConvertResult {
            conversion,
            fallbacks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_build_alias_index() {
        let dir = tempdir().unwrap();

        // Create a test Rd file
        let rd_content = r#"\name{my_func}
\alias{my_func}
\alias{my_func_alias}
\title{My Function}
\description{A test function}
"#;
        let rd_path = dir.path().join("my_func.Rd");
        fs::write(&rd_path, rd_content).unwrap();

        let files = vec![rd_path];
        let index = build_alias_index(&files, InputFormat::Rd).unwrap();

        assert_eq!(index.get("my_func"), Some(&"my_func".to_string()));
        assert_eq!(index.get("my_func_alias"), Some(&"my_func".to_string()));
    }

    #[test]
    fn test_rd_package_from_directory() {
        let dir = tempdir().unwrap();

        // Create test Rd files
        let rd1 = r#"\name{func_a}
\alias{func_a}
\alias{FuncA}
\title{Function A}
"#;
        let rd2 = r#"\name{func_b}
\alias{func_b}
\title{Function B}
"#;
        fs::write(dir.path().join("func_a.Rd"), rd1).unwrap();
        fs::write(dir.path().join("func_b.Rd"), rd2).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();

        assert_eq!(package.files.len(), 2);
        assert_eq!(package.resolve_alias("func_a"), Some("func_a"));
        assert_eq!(package.resolve_alias("FuncA"), Some("func_a"));
        assert_eq!(package.resolve_alias("func_b"), Some("func_b"));
        assert_eq!(package.resolve_alias("nonexistent"), None);
    }

    #[test]
    fn test_generate_topic_index() {
        let dir = tempdir().unwrap();

        // Create test Rd files - one with lifecycle, one without
        let rd_deprecated = r#"\name{old_func}
\alias{old_func}
\alias{legacy_func}
\title{Old Function}
\description{
\ifelse{html}{\href{https://lifecycle.r-lib.org/}{\figure{lifecycle-deprecated.svg}{}}}{\strong{[Deprecated]}}
An old deprecated function.
}
"#;
        let rd_normal = r#"\name{new_func}
\alias{new_func}
\title{New Function}
\description{A normal function.}
"#;
        fs::write(dir.path().join("old_func.Rd"), rd_deprecated).unwrap();
        fs::write(dir.path().join("new_func.Rd"), rd_normal).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = TopicIndexOptions {
            output_extension: "qmd".to_string(),
            include_internal: false,
        };
        let index = generate_topic_index(&package, &options).unwrap();

        assert_eq!(index.topics.len(), 2);

        // Topics are sorted by name
        let new_topic = index.topics.iter().find(|t| t.name == "new_func").unwrap();
        assert_eq!(new_topic.file, "new_func.qmd");
        assert_eq!(new_topic.title, "New Function");
        assert!(new_topic.metadata.aliases.contains(&"new_func".to_string()));
        assert!(new_topic.metadata.lifecycle.is_none());

        let old_topic = index.topics.iter().find(|t| t.name == "old_func").unwrap();
        assert_eq!(old_topic.file, "old_func.qmd");
        assert_eq!(old_topic.title, "Old Function");
        assert!(old_topic.metadata.aliases.contains(&"old_func".to_string()));
        assert!(
            old_topic
                .metadata
                .aliases
                .contains(&"legacy_func".to_string())
        );
        assert_eq!(old_topic.metadata.lifecycle, Some("deprecated".to_string()));

        // Both are hand-written, so no source_files
        assert!(new_topic.metadata.source_files.is_empty());
        assert!(old_topic.metadata.source_files.is_empty());
    }

    #[test]
    fn test_generate_topic_index_with_roxygen_sources() {
        let dir = tempdir().unwrap();

        // Create a roxygen2-generated Rd file with source files
        let rd_roxygen = r#"% Generated by roxygen2: do not edit by hand
% Please edit documentation in R/coord-map.R, R/coord-quickmap.R
\name{coord_map}
\alias{coord_map}
\alias{coord_quickmap}
\title{Map projections}
\description{Projects coordinates onto a map.}
"#;
        // Create a hand-written Rd file (no roxygen2 header)
        let rd_manual = r#"\name{manual}
\alias{manual}
\title{Manual Topic}
\description{Hand-written documentation.}
"#;
        fs::write(dir.path().join("coord_map.Rd"), rd_roxygen).unwrap();
        fs::write(dir.path().join("manual.Rd"), rd_manual).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = TopicIndexOptions {
            output_extension: "qmd".to_string(),
            include_internal: false,
        };
        let index = generate_topic_index(&package, &options).unwrap();

        assert_eq!(index.topics.len(), 2);

        // Roxygen-generated topic has source_files
        let coord_topic = index.topics.iter().find(|t| t.name == "coord_map").unwrap();
        assert_eq!(
            coord_topic.metadata.source_files,
            vec!["R/coord-map.R", "R/coord-quickmap.R"]
        );

        // Manual topic has no source_files
        let manual_topic = index.topics.iter().find(|t| t.name == "manual").unwrap();
        assert!(manual_topic.metadata.source_files.is_empty());
    }

    #[test]
    fn test_topic_index_json_serialization() {
        let index = TopicIndex {
            topics: vec![
                TopicInfo {
                    name: "foo".to_string(),
                    file: "foo.qmd".to_string(),
                    title: "Foo Function".to_string(),
                    metadata: RdMetadata {
                        lifecycle: Some("deprecated".to_string()),
                        aliases: vec!["foo".to_string(), "bar".to_string()],
                        keywords: vec![],
                        concepts: vec![],
                        source_files: vec!["R/foo.R".to_string(), "R/bar.R".to_string()],
                    },
                },
                TopicInfo {
                    name: "baz".to_string(),
                    file: "baz.qmd".to_string(),
                    title: "Baz Function".to_string(),
                    metadata: RdMetadata {
                        lifecycle: None,
                        aliases: vec!["baz".to_string()],
                        keywords: vec![],
                        concepts: vec![],
                        source_files: vec![], // Empty - should be omitted from JSON
                    },
                },
            ],
        };

        let json = index.to_json().unwrap();

        // Parse JSON to verify structure
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let topics = parsed["topics"].as_array().unwrap();
        assert_eq!(topics.len(), 2);

        // First topic has lifecycle and source_files (flattened from metadata)
        assert_eq!(topics[0]["name"], "foo");
        assert_eq!(topics[0]["lifecycle"], "deprecated");
        assert_eq!(
            topics[0]["source_files"],
            serde_json::json!(["R/foo.R", "R/bar.R"])
        );

        // Second topic has no lifecycle or source_files fields (skip_serializing_if)
        assert_eq!(topics[1]["name"], "baz");
        assert!(topics[1].get("lifecycle").is_none());
        assert!(topics[1].get("source_files").is_none());
    }

    // ========================================================================
    // PackageConverter Builder tests
    // ========================================================================

    #[test]
    fn test_package_converter_basic() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        // Create test Rd files
        let rd1 = r#"\name{alpha}
\alias{alpha}
\title{Alpha Function}
\description{The alpha function.}
"#;
        let rd2 = r#"\name{beta}
\alias{beta}
\title{Beta Function}
\description{The beta function.}
"#;
        fs::write(dir.path().join("alpha.Rd"), rd1).unwrap();
        fs::write(dir.path().join("beta.Rd"), rd2).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: true,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false,
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();

        assert_eq!(result.conversion.success_count, 2);
        assert!(result.conversion.failed_files.is_empty());
        assert_eq!(result.conversion.output_files.len(), 2);

        // Check output files exist
        assert!(out_dir.path().join("alpha.qmd").exists());
        assert!(out_dir.path().join("beta.qmd").exists());

        // Check content
        let alpha_content = fs::read_to_string(out_dir.path().join("alpha.qmd")).unwrap();
        assert!(alpha_content.contains("title: \"Alpha Function\""));
        assert!(!alpha_content.contains("\n# Alpha Function\n"));

        // Fallbacks should be empty when external links not used
        #[cfg(feature = "external-links")]
        assert!(result.fallbacks.is_empty());
    }

    #[test]
    fn test_package_converter_with_alias_resolution() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        // Create Rd files that reference each other
        let rd_main = r#"\name{main_func}
\alias{main_func}
\alias{mf}
\title{Main Function}
\description{See \link{helper_func} for details.}
"#;
        let rd_helper = r#"\name{helper_func}
\alias{helper_func}
\title{Helper Function}
\description{A helper for \link{mf}.}
"#;
        fs::write(dir.path().join("main_func.Rd"), rd_main).unwrap();
        fs::write(dir.path().join("helper_func.Rd"), rd_helper).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: false,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false,
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();
        assert_eq!(result.conversion.success_count, 2);

        // Check alias resolution works (links use [`text`](url) format)
        let main_content = fs::read_to_string(out_dir.path().join("main_func.qmd")).unwrap();
        assert!(main_content.contains("[`helper_func`](helper_func.qmd)"));

        let helper_content = fs::read_to_string(out_dir.path().join("helper_func.qmd")).unwrap();
        // "mf" alias should resolve to main_func
        assert!(helper_content.contains("[`mf`](main_func.qmd)"));
    }

    #[test]
    fn test_package_converter_md_output() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd = r#"\name{test}
\alias{test}
\title{Test}
\description{Test function.}
\examples{
x <- 1
}
"#;
        fs::write(dir.path().join("test.Rd"), rd).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "md".to_string(),
            frontmatter: true,
            pagetitle: true,
            quarto_code_blocks: false, // Plain markdown
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false,
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();
        assert_eq!(result.conversion.success_count, 1);

        // Check .md extension
        assert!(out_dir.path().join("test.md").exists());

        let content = fs::read_to_string(out_dir.path().join("test.md")).unwrap();
        // Should have pagetitle
        assert!(content.contains("pagetitle: \"Test — test\""));
        // Should use plain code blocks, not {r}
        assert!(content.contains("```r"));
        assert!(!content.contains("```{r}"));
    }

    #[test]
    fn test_package_converter_handles_parse_errors_at_load_time() {
        let dir = tempdir().unwrap();

        // One valid file
        let rd_good = r#"\name{good}
\alias{good}
\title{Good}
\description{Works fine.}
"#;
        // One invalid file (unclosed brace)
        let rd_bad = r#"\name{bad
\title{Bad}
"#;
        fs::write(dir.path().join("good.Rd"), rd_good).unwrap();
        fs::write(dir.path().join("bad.Rd"), rd_bad).unwrap();

        // from_directory fails when any file has parse errors (during alias index building)
        let result = RdPackage::from_directory(dir.path(), false);
        assert!(result.is_err());

        // The error should be a parse error
        let err = result.unwrap_err();
        assert!(err.to_string().contains("bad.Rd"));
    }

    #[test]
    fn test_package_converter_with_unqualified_link_url() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd = r#"\name{caller}
\alias{caller}
\title{Caller}
\description{Uses \link{unknown_external}.}
"#;
        fs::write(dir.path().join("caller.Rd"), rd).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: false,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: Some("https://rdrr.io/r/base/{topic}.html".to_string()),
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false,
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();
        assert_eq!(result.conversion.success_count, 1);

        let content = fs::read_to_string(out_dir.path().join("caller.qmd")).unwrap();
        // Link text has backticks
        assert!(
            content.contains("[`unknown_external`](https://rdrr.io/r/base/unknown_external.html)")
        );
    }

    #[test]
    fn test_package_converter_with_external_link_url() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd = r#"\name{caller}
\alias{caller}
\title{Caller}
\description{Uses \link{caller} and \link[somepkg]{something}.}
"#;
        fs::write(dir.path().join("caller.Rd"), rd).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: false,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false,
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();
        assert_eq!(result.conversion.success_count, 1);

        let content = fs::read_to_string(out_dir.path().join("caller.qmd")).unwrap();
        // Alias-resolved internal link uses the derived {file}.qmd template
        assert!(content.contains("[`caller`](caller.qmd)"));
        // Qualified link whose package is not in package_urls falls back to the template
        assert!(
            content
                .contains("[`somepkg::something`](https://rdrr.io/pkg/somepkg/man/something.html)")
        );
    }

    #[test]
    fn test_package_converter_with_package_urls() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd = r#"\name{tidyverse_user}
\alias{tidyverse_user}
\title{Tidyverse User}
\description{Uses \link[dplyr]{mutate} and \link[ggplot2]{ggplot}.}
"#;
        fs::write(dir.path().join("tidyverse_user.Rd"), rd).unwrap();

        let mut package_urls_map = std::collections::HashMap::new();
        package_urls_map.insert(
            "dplyr".to_string(),
            "https://dplyr.tidyverse.org/reference/{topic}.html".to_string(),
        );
        package_urls_map.insert(
            "ggplot2".to_string(),
            "https://ggplot2.tidyverse.org/reference/{topic}.html".to_string(),
        );

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: false,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: Some(package_urls_map),
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false,
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();
        assert_eq!(result.conversion.success_count, 1);

        let content = fs::read_to_string(out_dir.path().join("tidyverse_user.qmd")).unwrap();
        // Link text uses [`package::topic`] format
        assert!(
            content
                .contains("[`dplyr::mutate`](https://dplyr.tidyverse.org/reference/mutate.html)")
        );
        assert!(
            content.contains(
                "[`ggplot2::ggplot`](https://ggplot2.tidyverse.org/reference/ggplot.html)"
            )
        );
    }

    #[test]
    fn test_package_converter_package_urls_win_over_external_link_url() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd = r#"\name{wrapper}
\alias{wrapper}
\title{Wrapper}
\description{Uses \link[dplyr]{mutate} and \link[somepkg]{something}.}
"#;
        fs::write(dir.path().join("wrapper.Rd"), rd).unwrap();

        let mut package_urls_map = std::collections::HashMap::new();
        package_urls_map.insert(
            "dplyr".to_string(),
            "https://dplyr.tidyverse.org/reference/{topic}.html".to_string(),
        );

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            frontmatter: false,
            pagetitle: false,
            parallel_jobs: Some(1),
            package_urls: Some(package_urls_map),
            external_link_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
            ..Default::default()
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();
        assert_eq!(result.conversion.success_count, 1);

        let content = fs::read_to_string(out_dir.path().join("wrapper.qmd")).unwrap();
        // package_urls entry takes precedence over external_link_url
        assert!(
            content
                .contains("[`dplyr::mutate`](https://dplyr.tidyverse.org/reference/mutate.html)")
        );
        // Packages not in package_urls fall back to external_link_url
        assert!(
            content
                .contains("[`somepkg::something`](https://rdrr.io/pkg/somepkg/man/something.html)")
        );
    }

    #[cfg(feature = "external-links")]
    #[test]
    fn test_external_links_skip_user_covered_packages() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();
        let lib_dir = tempdir().unwrap(); // empty: nothing resolvable

        let rd = r#"\name{wrapper}
\alias{wrapper}
\title{Wrapper}
\description{Uses \link[dplyr]{mutate} and \link[somepkg]{something}.}
"#;
        fs::write(dir.path().join("wrapper.Rd"), rd).unwrap();

        let mut package_urls_map = std::collections::HashMap::new();
        package_urls_map.insert(
            "dplyr".to_string(),
            "https://dplyr.tidyverse.org/reference/{topic}.html".to_string(),
        );

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            frontmatter: false,
            pagetitle: false,
            parallel_jobs: Some(1),
            package_urls: Some(package_urls_map),
            external_link_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
            ..Default::default()
        };

        let result = PackageConverter::new(&package, options)
            .with_external_links(ExternalLinkOptions {
                lib_paths: vec![lib_dir.path().to_path_buf()],
                cache_dir: None,
            })
            .convert()
            .unwrap();
        assert_eq!(result.conversion.success_count, 1);

        // Packages covered by user-provided package_urls are not sent to the
        // resolver, so they must not show up as fallbacks
        assert!(!result.fallbacks.contains_key("dplyr"));
        assert_eq!(
            result.fallbacks.get("somepkg"),
            Some(&FallbackReason::NotInstalled)
        );

        let content = fs::read_to_string(out_dir.path().join("wrapper.qmd")).unwrap();
        assert!(
            content
                .contains("[`dplyr::mutate`](https://dplyr.tidyverse.org/reference/mutate.html)")
        );
        assert!(
            content
                .contains("[`somepkg::something`](https://rdrr.io/pkg/somepkg/man/something.html)")
        );
    }

    #[test]
    fn test_package_converter_internal_link_url_override() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd_main = r#"\name{main_func}
\alias{main_func}
\title{Main Function}
\description{See \link{helper_func} for details.}
"#;
        let rd_helper = r#"\name{helper_func}
\alias{helper_func}
\title{Helper Function}
\description{A helper.}
"#;
        fs::write(dir.path().join("main_func.Rd"), rd_main).unwrap();
        fs::write(dir.path().join("helper_func.Rd"), rd_helper).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            frontmatter: false,
            pagetitle: false,
            parallel_jobs: Some(1),
            // Override the derived {file}.qmd template
            internal_link_url: Some("/reference/{file}.html".to_string()),
            ..Default::default()
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();
        assert_eq!(result.conversion.success_count, 2);

        let content = fs::read_to_string(out_dir.path().join("main_func.qmd")).unwrap();
        assert!(content.contains("[`helper_func`](/reference/helper_func.html)"));
    }

    #[test]
    fn test_package_converter_empty_directory() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        assert!(package.files.is_empty());

        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: false,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false,
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();

        assert_eq!(result.conversion.success_count, 0);
        assert!(result.conversion.failed_files.is_empty());
        assert!(result.conversion.output_files.is_empty());
    }

    #[test]
    fn test_full_convert_result_structure() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd = r#"\name{simple}
\alias{simple}
\title{Simple}
\description{A simple function.}
"#;
        fs::write(dir.path().join("simple.Rd"), rd).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: false,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false,
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();

        // Check FullConvertResult fields
        assert_eq!(result.conversion.success_count, 1);
        assert!(result.conversion.failed_files.is_empty());
        assert_eq!(result.conversion.output_files.len(), 1);
        // Fallbacks are empty when not using external links feature
        #[cfg(feature = "external-links")]
        assert!(result.fallbacks.is_empty());
    }

    // ========================================================================
    // Internal topic skipping tests
    // ========================================================================

    #[test]
    fn test_internal_topics_skipped_by_default() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        // Create one public and one internal topic
        let rd_public = r#"\name{public_func}
\alias{public_func}
\title{Public Function}
\description{A public function.}
"#;
        let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
        fs::write(dir.path().join("public_func.Rd"), rd_public).unwrap();
        fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        assert_eq!(package.files.len(), 2);

        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: false,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: false, // Default: skip internal
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();

        // Only public topic should be converted
        assert_eq!(result.conversion.success_count, 1);
        assert_eq!(result.conversion.skipped_internal.len(), 1);
        assert!(result.conversion.failed_files.is_empty());

        // Check that only public_func.qmd was created
        assert!(out_dir.path().join("public_func.qmd").exists());
        assert!(!out_dir.path().join("internal_func.qmd").exists());

        // Check skipped file name
        assert!(
            result.conversion.skipped_internal[0]
                .to_string_lossy()
                .contains("internal_func.Rd")
        );
    }

    #[test]
    fn test_internal_topics_included_when_requested() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        // Create one public and one internal topic
        let rd_public = r#"\name{public_func}
\alias{public_func}
\title{Public Function}
\description{A public function.}
"#;
        let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
        fs::write(dir.path().join("public_func.Rd"), rd_public).unwrap();
        fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();

        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            output_extension: "qmd".to_string(),
            frontmatter: false,
            pagetitle: false,
            quarto_code_blocks: true,
            parallel_jobs: Some(1),
            internal_link_url: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            include_internal: true, // Include internal topics
            include_html_output: false,
            prefer_ascii_math: false,
            arguments_format: ArgumentsFormat::default(),
        };

        let result = PackageConverter::new(&package, options).convert().unwrap();

        // Both topics should be converted
        assert_eq!(result.conversion.success_count, 2);
        assert!(result.conversion.skipped_internal.is_empty());
        assert!(result.conversion.failed_files.is_empty());

        // Check that both files were created
        assert!(out_dir.path().join("public_func.qmd").exists());
        assert!(out_dir.path().join("internal_func.qmd").exists());
    }

    #[test]
    fn test_has_keyword_internal_detection() {
        // Test the has_keyword_internal helper function
        let rd_internal = r#"\name{func}
\keyword{internal}
\title{Test}
"#;
        let rd_normal = r#"\name{func}
\keyword{datasets}
\title{Test}
"#;
        let rd_no_keyword = r#"\name{func}
\title{Test}
"#;

        let doc_internal = rd2qmd_source::parse(rd_internal)
            .unwrap()
            .document()
            .clone();
        let doc_normal = rd2qmd_source::parse(rd_normal).unwrap().document().clone();
        let doc_no_keyword = rd2qmd_source::parse(rd_no_keyword)
            .unwrap()
            .document()
            .clone();

        assert!(has_keyword_internal(&doc_internal));
        assert!(!has_keyword_internal(&doc_normal));
        assert!(!has_keyword_internal(&doc_no_keyword));
    }

    #[test]
    fn test_topic_index_excludes_internal_by_default() {
        let dir = tempdir().unwrap();

        // Create public and internal topics
        let rd_public = r#"\name{public_func}
\alias{public_func}
\title{Public Function}
\description{A public function.}
"#;
        let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
        fs::write(dir.path().join("public_func.Rd"), rd_public).unwrap();
        fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();

        // Default: exclude internal
        let options = TopicIndexOptions {
            output_extension: "qmd".to_string(),
            include_internal: false,
        };
        let index = generate_topic_index(&package, &options).unwrap();

        // Only public topic should be in the index
        assert_eq!(index.topics.len(), 1);
        assert_eq!(index.topics[0].name, "public_func");
    }

    #[test]
    fn test_topic_index_includes_internal_when_requested() {
        let dir = tempdir().unwrap();

        // Create public and internal topics
        let rd_public = r#"\name{public_func}
\alias{public_func}
\title{Public Function}
\description{A public function.}
"#;
        let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
        fs::write(dir.path().join("public_func.Rd"), rd_public).unwrap();
        fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();

        // Include internal topics
        let options = TopicIndexOptions {
            output_extension: "qmd".to_string(),
            include_internal: true,
        };
        let index = generate_topic_index(&package, &options).unwrap();

        // Both topics should be in the index
        assert_eq!(index.topics.len(), 2);

        // Topics are sorted by name
        let names: Vec<_> = index.topics.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"public_func"));
        assert!(names.contains(&"internal_func"));
    }

    // ========================================================================
    // AST JSON export/import tests
    // ========================================================================

    #[test]
    fn test_export_package_ast() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd_roxygen = r#"% Generated by roxygen2: do not edit by hand
% Please edit documentation in R/coord-map.R
\name{coord_map}
\alias{coord_map}
\title{Map projections}
\description{Projects coordinates onto a map.}
"#;
        fs::write(dir.path().join("coord_map.Rd"), rd_roxygen).unwrap();

        let result = export_package_ast(dir.path(), false, out_dir.path(), Some(1)).unwrap();

        assert_eq!(result.success_count, 1);
        assert!(result.failed_files.is_empty());
        assert!(result.skipped_internal.is_empty());

        let json_path = out_dir.path().join("coord_map.json");
        assert!(json_path.exists());

        let content = fs::read_to_string(&json_path).unwrap();
        let envelope = RdAstEnvelope::from_json(&content).unwrap();
        assert_eq!(envelope.source, Some("coord_map.Rd".to_string()));
        assert_eq!(envelope.source_files, vec!["R/coord-map.R".to_string()]);
    }

    #[test]
    fn test_export_package_ast_exports_internal_topics_too() {
        let dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
        fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

        let result = export_package_ast(dir.path(), false, out_dir.path(), Some(1)).unwrap();

        assert_eq!(result.success_count, 1);
        assert!(out_dir.path().join("internal_func.json").exists());
    }

    #[test]
    fn test_package_from_directory_with_ast_json_format() {
        let dir = tempdir().unwrap();
        let json_dir = tempdir().unwrap();
        let out_dir = tempdir().unwrap();

        let rd_main = r#"\name{main_func}
\alias{main_func}
\title{Main Function}
\description{See \link{helper_func} for details.}
"#;
        let rd_helper = r#"\name{helper_func}
\alias{helper_func}
\title{Helper Function}
\description{A helper function.}
"#;
        fs::write(dir.path().join("main_func.Rd"), rd_main).unwrap();
        fs::write(dir.path().join("helper_func.Rd"), rd_helper).unwrap();

        export_package_ast(dir.path(), false, json_dir.path(), Some(1)).unwrap();

        let package =
            RdPackage::from_directory_with_format(json_dir.path(), false, InputFormat::AstJson)
                .unwrap();
        assert_eq!(package.files.len(), 2);
        assert_eq!(package.resolve_alias("main_func"), Some("main_func"));
        assert_eq!(package.resolve_alias("helper_func"), Some("helper_func"));

        let options = PackageConvertOptions {
            output_dir: out_dir.path().to_path_buf(),
            frontmatter: false,
            pagetitle: false,
            parallel_jobs: Some(1),
            ..Default::default()
        };
        let result = PackageConverter::new(&package, options).convert().unwrap();
        assert_eq!(result.conversion.success_count, 2);

        let main_content = fs::read_to_string(out_dir.path().join("main_func.qmd")).unwrap();
        assert!(main_content.contains("[`helper_func`](helper_func.qmd)"));
    }

    #[test]
    fn test_topic_index_from_ast_json_format() {
        let dir = tempdir().unwrap();
        let json_dir = tempdir().unwrap();

        let rd_roxygen = r#"% Generated by roxygen2: do not edit by hand
% Please edit documentation in R/coord-map.R
\name{coord_map}
\alias{coord_map}
\title{Map projections}
\description{Projects coordinates onto a map.}
"#;
        fs::write(dir.path().join("coord_map.Rd"), rd_roxygen).unwrap();

        export_package_ast(dir.path(), false, json_dir.path(), Some(1)).unwrap();

        let package =
            RdPackage::from_directory_with_format(json_dir.path(), false, InputFormat::AstJson)
                .unwrap();
        let options = TopicIndexOptions {
            output_extension: "qmd".to_string(),
            include_internal: false,
        };
        let index = generate_topic_index(&package, &options).unwrap();

        assert_eq!(index.topics.len(), 1);
        assert_eq!(index.topics[0].name, "coord_map");
        assert_eq!(
            index.topics[0].metadata.source_files,
            vec!["R/coord-map.R".to_string()]
        );
    }

    #[test]
    fn test_ast_json_version_mismatch_reported_as_parse_error() {
        let json_dir = tempdir().unwrap();
        fs::write(
            json_dir.path().join("broken.json"),
            r#"{"version":99,"source":null,"sourceFiles":[],"document":{"sections":[]}}"#,
        )
        .unwrap();

        let result =
            RdPackage::from_directory_with_format(json_dir.path(), false, InputFormat::AstJson);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("broken.json"));
    }
}
