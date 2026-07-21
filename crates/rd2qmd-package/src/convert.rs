//! Single-document and whole-package conversion pipeline

use rayon::prelude::*;
use rd2qmd_core::{
    ArgumentsFormat, Frontmatter, RdAstEnvelope, RdToMdastOptions, WriterOptions,
    extract_rd_metadata, extract_text, mdast_to_qmd, rd_to_mdast_with_options,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{PackageError, Result};
use crate::package::{InputFormat, RdPackage, collect_files, load_document};

/// Internal error type for single-file conversion
///
/// Used within [`convert_single_file`] to distinguish between
/// files that should be skipped (internal) and actual errors.
#[derive(Debug)]
enum ConvertError {
    /// File has `\keyword{internal}` and should be skipped
    SkipInternal(Vec<rd2qmd_source::Diagnostic>),
    /// Conversion failed with an error message. Carries any diagnostics
    /// collected before the failure (empty if the file never parsed).
    Failed(String, Vec<rd2qmd_source::Diagnostic>),
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
    /// Recoverable parser diagnostics, grouped by input file.
    pub diagnostics: Vec<FileDiagnostics>,
}

/// Recoverable parser diagnostics associated with one source file.
#[derive(Debug)]
pub struct FileDiagnostics {
    pub file: PathBuf,
    pub diagnostics: Vec<rd2qmd_source::Diagnostic>,
}

/// Outcome of converting a single file
pub(crate) enum ConvertOutcome {
    /// Successfully converted, contains output path
    Success(PathBuf, PathBuf, Vec<rd2qmd_source::Diagnostic>),
    /// Skipped because the topic has \keyword{internal}
    SkippedInternal(PathBuf, Vec<rd2qmd_source::Diagnostic>),
    /// Failed to convert, contains input path, error message, and any
    /// diagnostics collected before the failure
    Failed(PathBuf, String, Vec<rd2qmd_source::Diagnostic>),
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
    let mut diagnostics = Vec::new();

    for result in results {
        match result {
            ConvertOutcome::Success(input_path, output_path, file_diagnostics) => {
                success_count += 1;
                output_files.push(output_path);
                if !file_diagnostics.is_empty() {
                    diagnostics.push(FileDiagnostics {
                        file: input_path,
                        diagnostics: file_diagnostics,
                    });
                }
            }
            ConvertOutcome::SkippedInternal(input_path, file_diagnostics) => {
                skipped_internal.push(input_path);
                if !file_diagnostics.is_empty() {
                    diagnostics.push(FileDiagnostics {
                        file: skipped_internal.last().unwrap().clone(),
                        diagnostics: file_diagnostics,
                    });
                }
            }
            ConvertOutcome::Failed(path, error, file_diagnostics) => {
                if !file_diagnostics.is_empty() {
                    diagnostics.push(FileDiagnostics {
                        file: path.clone(),
                        diagnostics: file_diagnostics,
                    });
                }
                failed_files.push((path, error));
            }
        }
    }

    Ok(ConvertResult {
        success_count,
        failed_files,
        output_files,
        skipped_internal,
        diagnostics,
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
    let mut diagnostics = Vec::new();

    for (file, result) in files.iter().zip(results) {
        match result {
            Ok((output_path, file_diagnostics)) => {
                success_count += 1;
                output_files.push(output_path);
                if !file_diagnostics.is_empty() {
                    diagnostics.push(FileDiagnostics {
                        file: file.clone(),
                        diagnostics: file_diagnostics,
                    });
                }
            }
            Err((input_path, message, file_diagnostics)) => {
                if !file_diagnostics.is_empty() {
                    diagnostics.push(FileDiagnostics {
                        file: input_path.clone(),
                        diagnostics: file_diagnostics,
                    });
                }
                failed_files.push((input_path, message));
            }
        }
    }

    Ok(ConvertResult {
        success_count,
        failed_files,
        output_files,
        skipped_internal: Vec::new(),
        diagnostics,
    })
}

/// Parse a single Rd file and write it as an AST JSON envelope
#[allow(clippy::type_complexity)]
pub(crate) fn export_single_file(
    input: &Path,
    root: &Path,
    output_dir: &Path,
) -> std::result::Result<
    (PathBuf, Vec<rd2qmd_source::Diagnostic>),
    (PathBuf, String, Vec<rd2qmd_source::Diagnostic>),
> {
    let export = || -> std::result::Result<
        (PathBuf, Vec<rd2qmd_source::Diagnostic>),
        (String, Vec<rd2qmd_source::Diagnostic>),
    > {
        let content = fs::read_to_string(input).map_err(|e| (e.to_string(), Vec::new()))?;

        let parsed = rd2qmd_source::parse(&content)
            .map_err(|e| (format!("Parse error: {e}"), Vec::new()))?;
        let (doc, diagnostics) = parsed.into_parts();
        let source_files = extract_rd_metadata(&doc).source_files;

        let source = input
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        let envelope = RdAstEnvelope::new(doc, source, source_files);
        let json = envelope
            .to_json_pretty()
            .map_err(|e| (e.to_string(), diagnostics.clone()))?;

        // Determine output path
        let relative = input.strip_prefix(root).unwrap_or(input);
        let output_path = output_dir.join(relative).with_extension("json");

        // Create parent directory if needed
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| (e.to_string(), diagnostics.clone()))?;
        }

        // Write output
        fs::write(&output_path, json).map_err(|e| (e.to_string(), diagnostics.clone()))?;

        Ok((output_path, diagnostics))
    };

    export().map_err(|(message, diagnostics)| (input.to_path_buf(), message, diagnostics))
}

/// Check if a document has \keyword{internal}
pub(crate) fn has_keyword_internal(doc: &rd2qmd_core::RdDocument) -> bool {
    doc.keywords()
        .any(|keyword| keyword.eq_ignore_ascii_case("internal"))
}

/// Convert a single Rd file
pub(crate) fn convert_single_file(
    input: &Path,
    package: &RdPackage,
    options: &PackageConvertOptions,
) -> ConvertOutcome {
    let convert =
        || -> std::result::Result<(PathBuf, Vec<rd2qmd_source::Diagnostic>), ConvertError> {
            // Read and parse the input file (Rd or AST JSON, depending on package.format)
            let (doc, source_files, diagnostics) = load_document(input, package.format)
                .map_err(|e| ConvertError::Failed(e.to_string(), Vec::new()))?;

            // Check for \keyword{internal} - skip unless include_internal is set
            if !options.include_internal && has_keyword_internal(&doc) {
                return Err(ConvertError::SkipInternal(diagnostics));
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
                fs::create_dir_all(parent)
                    .map_err(|e| ConvertError::Failed(e.to_string(), diagnostics.clone()))?;
            }

            // Write output
            fs::write(&output_path, qmd)
                .map_err(|e| ConvertError::Failed(e.to_string(), diagnostics.clone()))?;

            Ok((output_path, diagnostics))
        };

    match convert() {
        Ok((path, diagnostics)) => ConvertOutcome::Success(input.to_path_buf(), path, diagnostics),
        Err(ConvertError::SkipInternal(diagnostics)) => {
            ConvertOutcome::SkippedInternal(input.to_path_buf(), diagnostics)
        }
        Err(ConvertError::Failed(msg, diagnostics)) => {
            ConvertOutcome::Failed(input.to_path_buf(), msg, diagnostics)
        }
    }
}
