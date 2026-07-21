//! Package discovery: locating and loading an R package's documentation files

use rd2qmd_core::{RdAstEnvelope, RdDocument, extract_rd_metadata, extract_text};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{PackageError, Result};

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
    pub(crate) root: PathBuf,
    /// List of Rd files in the package
    pub(crate) files: Vec<PathBuf>,
    /// Alias index: maps alias names to Rd file basenames (without extension)
    pub(crate) alias_index: HashMap<String, String>,
    /// Format of the files in this package
    pub(crate) format: InputFormat,
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

/// Collect all documentation files in a directory
///
/// Scans for `.Rd`/`.rd` files with [`InputFormat::Rd`], or `.json` files
/// with [`InputFormat::AstJson`].
pub(crate) fn collect_files(
    dir: &Path,
    recursive: bool,
    format: InputFormat,
) -> Result<Vec<PathBuf>> {
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
pub(crate) fn load_document(
    path: &Path,
    format: InputFormat,
) -> Result<(RdDocument, Vec<String>, Vec<rd2qmd_source::Diagnostic>)> {
    let content = fs::read_to_string(path)?;

    match format {
        InputFormat::Rd => {
            let parsed = rd2qmd_source::parse(&content).map_err(|e| PackageError::Parse {
                file: path.to_path_buf(),
                message: e.to_string(),
            })?;
            let (doc, diagnostics) = parsed.into_parts();
            let source_files = extract_rd_metadata(&doc).source_files;
            Ok((doc, source_files, diagnostics))
        }
        InputFormat::AstJson => {
            let envelope = RdAstEnvelope::from_json(&content).map_err(|e| PackageError::Parse {
                file: path.to_path_buf(),
                message: e.to_string(),
            })?;
            Ok((envelope.document, envelope.source_files, Vec::new()))
        }
    }
}

/// Build an alias index from a list of documentation files
///
/// Returns a HashMap mapping alias names to file basenames (without extension)
pub(crate) fn build_alias_index(
    files: &[PathBuf],
    format: InputFormat,
) -> Result<HashMap<String, String>> {
    let mut index = HashMap::new();

    for file in files {
        let basename = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let (doc, _source_files, _diagnostics) = load_document(file, format)?;

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
