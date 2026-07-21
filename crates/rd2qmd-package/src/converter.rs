// ============================================================================
// Package Converter Builder
// ============================================================================

use std::collections::HashMap;
#[cfg(feature = "external-links")]
use std::path::PathBuf;

use crate::convert::{ConvertResult, PackageConvertOptions, convert_package};
use crate::error::{FallbackReason, Result};
use crate::package::RdPackage;

#[cfg(feature = "external-links")]
use crate::external_links::{
    PackageUrlResolver, PackageUrlResolverOptions, collect_external_packages,
};

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
