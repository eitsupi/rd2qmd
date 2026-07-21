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

mod convert;
mod converter;
mod error;
mod package;
mod topic_index;

#[cfg(test)]
mod tests;

pub use convert::{
    ConvertResult, FileDiagnostics, PackageConvertOptions, convert_package, export_package_ast,
};
#[cfg(feature = "external-links")]
pub use converter::ExternalLinkOptions;
pub use converter::{FullConvertResult, PackageConverter};
pub use error::{FallbackReason, PackageError, Result};
pub use package::{InputFormat, RdPackage};
// `external_links.rs` reaches into `crate::load_document` directly; keep it
// resolvable at the crate root without widening its real (crate-internal)
// visibility.
#[cfg(feature = "external-links")]
pub(crate) use package::load_document;
pub use topic_index::{
    TopicIndex, TopicIndexOptions, TopicIndexResult, TopicInfo, generate_topic_index,
    generate_topic_index_with_diagnostics,
};
