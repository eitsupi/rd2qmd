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
    FallbackReason, PackageResolveResult, PackageUrlResolver, PackageUrlResolverOptions,
    collect_external_packages,
};

use rayon::prelude::*;
use rd_parser::Lifecycle;
use rd2qmd_core::{
    ConverterOptions, Frontmatter, RdNode, SectionTag, WriterOptions, mdast_to_qmd, parse,
    rd_to_mdast_with_options,
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

/// Information about an R package's documentation
#[derive(Debug, Clone)]
pub struct RdPackage {
    /// Root directory containing Rd files
    pub root: PathBuf,
    /// List of Rd files in the package
    pub files: Vec<PathBuf>,
    /// Alias index: maps alias names to Rd file basenames (without extension)
    pub alias_index: HashMap<String, String>,
}

impl RdPackage {
    /// Load a package from a directory containing Rd files
    ///
    /// This scans the directory for .Rd files and builds an alias index
    /// by parsing each file and extracting \alias{} tags.
    pub fn from_directory(path: &Path, recursive: bool) -> Result<Self> {
        if !path.is_dir() {
            return Err(PackageError::DirectoryNotFound(path.to_path_buf()));
        }

        let files = collect_rd_files(path, recursive)?;
        let alias_index = build_alias_index(&files)?;

        Ok(Self {
            root: path.to_path_buf(),
            files,
            alias_index,
        })
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
    /// Whether to add pagetitle in pkgdown style ("<title> — <name>")
    pub pagetitle: bool,
    /// Whether to use Quarto {r} code blocks for examples
    pub quarto_code_blocks: bool,
    /// Number of parallel jobs (None = use all CPUs)
    pub parallel_jobs: Option<usize>,
    /// URL pattern for unresolved links (fallback to base R documentation)
    /// Use `{topic}` as placeholder for the topic name.
    /// Example: "https://rdrr.io/r/base/{topic}.html"
    /// If None, unresolved links become inline code instead of hyperlinks
    pub unresolved_link_url: Option<String>,
    /// External package URL map: package name -> reference documentation base URL
    /// Used for resolving `\link[pkg]{topic}` patterns to actual URLs.
    /// Example: {"dplyr" -> "https://dplyr.tidyverse.org/reference"}
    pub external_package_urls: Option<HashMap<String, String>>,
    /// Make \dontrun{} example code executable (default: false)
    /// Matches pkgdown semantics: \dontrun{} means "never run this code"
    pub exec_dontrun: bool,
    /// Make \donttest{} example code executable (default: true)
    /// Matches pkgdown semantics: \donttest{} means "don't run during testing"
    /// but the code should normally be executable
    pub exec_donttest: bool,
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
            unresolved_link_url: None,
            external_package_urls: None,
            exec_dontrun: false,
            exec_donttest: true, // pkgdown-compatible: \donttest{} is executable by default
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
    /// Aliases for this topic (from \alias{})
    pub aliases: Vec<String>,
    /// Lifecycle stage, if present
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<Lifecycle>,
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
        match extract_topic_info(file, &options.output_extension) {
            Ok(info) => topics.push(info),
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
fn extract_topic_info(file: &Path, output_extension: &str) -> Result<TopicInfo> {
    let content = fs::read_to_string(file)?;
    let doc = parse(&content).map_err(|e| PackageError::Parse {
        file: file.to_path_buf(),
        message: e.to_string(),
    })?;

    // Extract name
    let name = doc
        .get_section(&SectionTag::Name)
        .map(|s| extract_text(&s.content))
        .unwrap_or_default();

    // Extract title
    let title = doc
        .get_section(&SectionTag::Title)
        .map(|s| extract_text(&s.content))
        .unwrap_or_default();

    // Extract aliases
    let mut aliases: Vec<String> = doc
        .get_sections(&SectionTag::Alias)
        .iter()
        .map(|s| extract_text(&s.content))
        .filter(|s| !s.is_empty())
        .collect();
    aliases.sort();
    aliases.dedup();

    // Extract lifecycle
    let lifecycle = doc.lifecycle();

    // Determine output filename
    let basename = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let output_file = format!("{}.{}", basename, output_extension);

    Ok(TopicInfo {
        name,
        file: output_file,
        title,
        aliases,
        lifecycle,
    })
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

    for result in results {
        match result {
            Ok(output_path) => {
                success_count += 1;
                output_files.push(output_path);
            }
            Err((path, error)) => {
                failed_files.push((path, error));
            }
        }
    }

    Ok(ConvertResult {
        success_count,
        failed_files,
        output_files,
    })
}

/// Convert a single Rd file
fn convert_single_file(
    input: &Path,
    package: &RdPackage,
    options: &PackageConvertOptions,
) -> std::result::Result<PathBuf, (PathBuf, String)> {
    let convert = || -> std::result::Result<PathBuf, Box<dyn std::error::Error>> {
        // Read input file
        let content = fs::read_to_string(input)?;

        // Parse Rd
        let doc = parse(&content).map_err(|e| format!("Parse error: {}", e))?;

        // Build converter options with alias map
        let converter_options = ConverterOptions {
            link_extension: Some(options.output_extension.clone()),
            alias_map: Some(package.alias_index.clone()),
            unresolved_link_url: options.unresolved_link_url.clone(),
            external_package_urls: options.external_package_urls.clone(),
            exec_dontrun: options.exec_dontrun,
            exec_donttest: options.exec_donttest,
            quarto_code_blocks: options.quarto_code_blocks,
            ..Default::default()
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
        let pagetitle = if options.pagetitle {
            match (&title, &name) {
                (Some(t), Some(n)) => Some(format!("{} \u{2014} {}", t, n)),
                _ => None,
            }
        } else {
            None
        };

        // Build writer options
        let writer_options = WriterOptions {
            frontmatter: if options.frontmatter {
                Some(Frontmatter {
                    title,
                    pagetitle,
                    format: None,
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
            fs::create_dir_all(parent)?;
        }

        // Write output
        fs::write(&output_path, qmd)?;

        Ok(output_path)
    };

    convert().map_err(|e| (input.to_path_buf(), e.to_string()))
}

/// Collect all .Rd files in a directory
fn collect_rd_files(dir: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension()
                && ext.eq_ignore_ascii_case("rd")
            {
                files.push(path);
            }
        } else if path.is_dir() && recursive {
            files.extend(collect_rd_files(&path, recursive)?);
        }
    }

    Ok(files)
}

/// Build an alias index from a list of Rd files
///
/// Returns a HashMap mapping alias names to Rd file basenames (without extension)
fn build_alias_index(files: &[PathBuf]) -> Result<HashMap<String, String>> {
    let mut index = HashMap::new();

    for file in files {
        let basename = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        // Parse the file to extract aliases
        let content = fs::read_to_string(file)?;
        let doc = parse(&content).map_err(|e| PackageError::Parse {
            file: file.clone(),
            message: e.to_string(),
        })?;

        // Extract all \alias{} sections
        let alias_sections = doc.get_sections(&SectionTag::Alias);
        for section in alias_sections {
            let alias = extract_text(&section.content).trim().to_string();
            if !alias.is_empty() {
                index.insert(alias, basename.clone());
            }
        }

        // Also add \name{} as an alias (it's always a valid reference)
        if let Some(name_section) = doc.get_section(&SectionTag::Name) {
            let name = extract_text(&name_section.content).trim().to_string();
            if !name.is_empty() {
                index.insert(name, basename.clone());
            }
        }
    }

    Ok(index)
}

/// Extract plain text from Rd nodes
fn extract_text(nodes: &[RdNode]) -> String {
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
        let index = build_alias_index(&files).unwrap();

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
        };
        let index = generate_topic_index(&package, &options).unwrap();

        assert_eq!(index.topics.len(), 2);

        // Topics are sorted by name
        let new_topic = index.topics.iter().find(|t| t.name == "new_func").unwrap();
        assert_eq!(new_topic.file, "new_func.qmd");
        assert_eq!(new_topic.title, "New Function");
        assert!(new_topic.aliases.contains(&"new_func".to_string()));
        assert!(new_topic.lifecycle.is_none());

        let old_topic = index.topics.iter().find(|t| t.name == "old_func").unwrap();
        assert_eq!(old_topic.file, "old_func.qmd");
        assert_eq!(old_topic.title, "Old Function");
        assert!(old_topic.aliases.contains(&"old_func".to_string()));
        assert!(old_topic.aliases.contains(&"legacy_func".to_string()));
        assert_eq!(old_topic.lifecycle, Some(Lifecycle::Deprecated));
    }

    #[test]
    fn test_topic_index_json_serialization() {
        let index = TopicIndex {
            topics: vec![
                TopicInfo {
                    name: "foo".to_string(),
                    file: "foo.qmd".to_string(),
                    title: "Foo Function".to_string(),
                    aliases: vec!["foo".to_string(), "bar".to_string()],
                    lifecycle: Some(Lifecycle::Deprecated),
                },
                TopicInfo {
                    name: "baz".to_string(),
                    file: "baz.qmd".to_string(),
                    title: "Baz Function".to_string(),
                    aliases: vec!["baz".to_string()],
                    lifecycle: None,
                },
            ],
        };

        let json = index.to_json().unwrap();

        // Parse JSON to verify structure
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let topics = parsed["topics"].as_array().unwrap();
        assert_eq!(topics.len(), 2);

        // First topic has lifecycle
        assert_eq!(topics[0]["name"], "foo");
        assert_eq!(topics[0]["lifecycle"], "deprecated");

        // Second topic has no lifecycle field (skip_serializing_if)
        assert_eq!(topics[1]["name"], "baz");
        assert!(topics[1].get("lifecycle").is_none());
    }
}
