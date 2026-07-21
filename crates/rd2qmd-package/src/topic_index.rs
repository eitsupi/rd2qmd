//! Topic index generation for package documentation

use rd2qmd_core::{RdMetadata, extract_rd_metadata, extract_text};
use serde::Serialize;
use std::path::Path;

use crate::convert::FileDiagnostics;
use crate::error::{PackageError, Result};
use crate::package::{InputFormat, RdPackage, load_document};

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

/// Result of topic index generation, including recoverable parser diagnostics.
#[derive(Debug)]
pub struct TopicIndexResult {
    pub index: TopicIndex,
    pub diagnostics: Vec<FileDiagnostics>,
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
    Ok(generate_topic_index_with_diagnostics(package, options)?.index)
}

/// Generate a topic index and retain recoverable parser diagnostics.
pub fn generate_topic_index_with_diagnostics(
    package: &RdPackage,
    options: &TopicIndexOptions,
) -> Result<TopicIndexResult> {
    let mut topics = Vec::new();
    let mut diagnostics = Vec::new();

    for file in &package.files {
        match extract_topic_info_with_diagnostics(file, &options.output_extension, package.format) {
            Ok((info, file_diagnostics)) => {
                if !file_diagnostics.is_empty() {
                    diagnostics.push(FileDiagnostics {
                        file: file.clone(),
                        diagnostics: file_diagnostics,
                    });
                }
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

    Ok(TopicIndexResult {
        index: TopicIndex { topics },
        diagnostics,
    })
}

fn extract_topic_info_with_diagnostics(
    file: &Path,
    output_extension: &str,
    format: InputFormat,
) -> Result<(TopicInfo, Vec<rd2qmd_source::Diagnostic>)> {
    let (doc, source_files, diagnostics) = load_document(file, format)?;

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

    Ok((
        TopicInfo {
            name,
            file: output_file,
            title,
            metadata,
        },
        diagnostics,
    ))
}
