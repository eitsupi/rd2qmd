//! Error and result types for package operations

use std::path::PathBuf;

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
