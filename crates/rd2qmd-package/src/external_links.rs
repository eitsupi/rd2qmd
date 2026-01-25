//! External link resolution for R package documentation
//!
//! This module provides functionality to resolve `\link[pkg]{topic}` patterns
//! to actual documentation URLs by:
//! 1. Looking up packages in R library paths
//! 2. Reading DESCRIPTION files to get documentation URLs
//! 3. Fetching pkgdown.yml to get reference documentation base URLs
//! 4. Caching results for performance
//!
//! The URL resolution logic is based on downlit's implementation:
//! - <https://github.com/r-lib/downlit/blob/main/R/metadata.R>
//! - <https://github.com/r-lib/downlit/blob/main/R/link.R>

use r_description::RDescription;
use saphyr::{LoadableYamlNode, Yaml};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::RdPackage;
use rd2qmd_core::RdNode;

/// Reason why a fallback URL was used for a package
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackReason {
    /// Package is not installed in any of the library paths
    NotInstalled,
    /// Package is installed but no pkgdown site could be found
    NoPkgdownSite,
}

/// Result of resolving external package URLs
#[derive(Debug, Clone, Default)]
pub struct PackageResolveResult {
    /// Map of package name to resolved reference URL
    pub urls: HashMap<String, String>,
    /// Packages that used fallback URLs, with the reason
    pub fallbacks: HashMap<String, FallbackReason>,
}

/// Options for package URL resolution
#[derive(Debug, Clone)]
pub struct PackageUrlResolverOptions {
    /// R library paths to search for packages
    pub lib_paths: Vec<PathBuf>,
    /// Cache directory for pkgdown.yml files (default: system temp)
    pub cache_dir: Option<PathBuf>,
    /// Fallback URL pattern for packages without pkgdown
    /// Use `{package}` and `{topic}` as placeholders
    /// Default: "https://rdrr.io/pkg/{package}/man/{topic}.html"
    pub fallback_url: Option<String>,
    /// Enable HTTP fetching of pkgdown.yml (can be disabled for offline mode)
    pub enable_http: bool,
}

impl Default for PackageUrlResolverOptions {
    fn default() -> Self {
        Self {
            lib_paths: Vec::new(),
            cache_dir: None,
            fallback_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
            enable_http: true,
        }
    }
}

/// Resolver for external R package documentation URLs
///
/// This struct caches resolved URLs to avoid repeated lookups.
pub struct PackageUrlResolver {
    options: PackageUrlResolverOptions,
    /// Cache: package name -> Option<reference base URL>
    /// None means we tried and failed to find a URL
    cache: HashMap<String, Option<String>>,
}

impl PackageUrlResolver {
    /// Create a new resolver with the given options
    pub fn new(options: PackageUrlResolverOptions) -> Self {
        Self {
            options,
            cache: HashMap::new(),
        }
    }

    /// Resolve a package to its reference documentation base URL
    ///
    /// Returns the base URL for reference docs (e.g., "https://dplyr.tidyverse.org/reference")
    /// or None if the package cannot be resolved.
    pub fn resolve(&mut self, package: &str) -> Option<String> {
        // Check cache first
        if let Some(cached) = self.cache.get(package) {
            return cached.clone();
        }

        // Try to resolve
        let result = self.resolve_uncached(package);
        self.cache.insert(package.to_string(), result.clone());
        result
    }

    /// Resolve without using cache
    fn resolve_uncached(&self, package: &str) -> Option<String> {
        // Find package directory in lib paths
        let pkg_dir = self.find_package_dir(package)?;

        // Check for local pkgdown.yml first
        let local_pkgdown = pkg_dir.join("pkgdown.yml");
        if local_pkgdown.exists()
            && let Some(url) = self.parse_pkgdown_yml(&local_pkgdown)
        {
            return Some(url);
        }

        // Read DESCRIPTION to get URL
        let desc_path = pkg_dir.join("DESCRIPTION");
        if !desc_path.exists() {
            return None;
        }

        let urls = self.get_urls_from_description(&desc_path)?;

        // Try to fetch pkgdown.yml from each URL
        if self.options.enable_http {
            for base_url in &urls {
                if let Some(url) = self.fetch_pkgdown_yml(base_url) {
                    return Some(url);
                }
            }
        }

        // Fallback: construct from first URL if it looks like a pkgdown site
        if let Some(first_url) = urls.first() {
            // Assume it's a pkgdown site and construct reference URL
            let reference_url = format!("{}/reference", first_url.trim_end_matches('/'));
            return Some(reference_url);
        }

        None
    }

    /// Find package directory in lib paths
    fn find_package_dir(&self, package: &str) -> Option<PathBuf> {
        for lib_path in &self.options.lib_paths {
            let pkg_dir = lib_path.join(package);
            if pkg_dir.is_dir() {
                return Some(pkg_dir);
            }
        }
        None
    }

    /// Get URLs from DESCRIPTION file
    fn get_urls_from_description(&self, desc_path: &Path) -> Option<Vec<String>> {
        let content = fs::read_to_string(desc_path).ok()?;
        let desc: RDescription = content.parse().ok()?;

        let urls: Vec<String> = desc
            .url?
            .into_iter()
            .map(|entry| entry.url.to_string())
            .collect();

        if urls.is_empty() { None } else { Some(urls) }
    }

    /// Parse a local pkgdown.yml file
    fn parse_pkgdown_yml(&self, path: &Path) -> Option<String> {
        let content = fs::read_to_string(path).ok()?;
        self.extract_reference_url_from_yaml(&content)
    }

    /// Fetch pkgdown.yml from a URL and extract reference URL
    fn fetch_pkgdown_yml(&self, base_url: &str) -> Option<String> {
        let pkgdown_url = format!("{}/pkgdown.yml", base_url.trim_end_matches('/'));

        // Check cache directory first
        if let Some(cache_dir) = &self.options.cache_dir {
            let cache_file = self.cache_path_for_url(cache_dir, &pkgdown_url);
            if cache_file.exists()
                && let Ok(content) = fs::read_to_string(&cache_file)
            {
                return self
                    .extract_reference_url_from_yaml(&content)
                    .or_else(|| Some(format!("{}/reference", base_url.trim_end_matches('/'))));
            }
        }

        // Fetch from network
        let content = self.http_get(&pkgdown_url)?;

        // Cache the result
        if let Some(cache_dir) = &self.options.cache_dir {
            let cache_file = self.cache_path_for_url(cache_dir, &pkgdown_url);
            if let Some(parent) = cache_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&cache_file, &content);
        }

        self.extract_reference_url_from_yaml(&content)
            .or_else(|| Some(format!("{}/reference", base_url.trim_end_matches('/'))))
    }

    /// Generate cache path for a URL
    fn cache_path_for_url(&self, cache_dir: &Path, url: &str) -> PathBuf {
        // Simple hash-based cache path
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();
        cache_dir.join(format!("pkgdown_{:x}.yml", hash))
    }

    /// Extract reference URL from pkgdown.yml content
    fn extract_reference_url_from_yaml(&self, content: &str) -> Option<String> {
        let docs = Yaml::load_from_str(content).ok()?;
        let doc = docs.first()?;

        // Try urls.reference first
        if let Some(urls) = doc.as_mapping_get("urls")
            && let Some(reference) = urls.as_mapping_get("reference")
            && let Some(s) = reference.as_str()
        {
            return Some(s.to_string());
        }

        // Try url field and construct reference path
        if let Some(url_node) = doc.as_mapping_get("url")
            && let Some(url) = url_node.as_str()
        {
            return Some(format!("{}/reference", url.trim_end_matches('/')));
        }

        None
    }

    /// Simple HTTP GET request
    fn http_get(&self, url: &str) -> Option<String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .ok()?;

        let response = client.get(url).send().ok()?;

        if !response.status().is_success() {
            return None;
        }

        response.text().ok()
    }

    /// Build a URL map for external packages
    ///
    /// Takes a set of package names and returns a map of package -> reference URL,
    /// along with information about which packages used fallback URLs.
    pub fn resolve_packages(&mut self, packages: &HashSet<String>) -> PackageResolveResult {
        let mut result = PackageResolveResult::default();

        for package in packages {
            // Check if package is installed first
            let is_installed = self.find_package_dir(package).is_some();

            if let Some(url) = self.resolve(package) {
                result.urls.insert(package.clone(), url);
                // Note: If resolve() succeeded, the package was installed and had a pkgdown site
            } else if let Some(pattern) = &self.options.fallback_url {
                // Use fallback URL pattern
                // Extract base URL by removing {topic}.html part
                // e.g., "https://rdrr.io/pkg/{package}/man/{topic}.html"
                //    -> "https://rdrr.io/pkg/dplyr/man"
                let base_url = pattern
                    .replace("{package}", package)
                    .replace("{topic}.html", "")
                    .replace("{topic}", "")
                    .trim_end_matches('/')
                    .to_string();
                result.urls.insert(package.clone(), base_url);

                // Track the fallback reason
                let reason = if is_installed {
                    FallbackReason::NoPkgdownSite
                } else {
                    FallbackReason::NotInstalled
                };
                result.fallbacks.insert(package.clone(), reason);
            }
        }

        result
    }

    /// Generate a full URL for a topic in a package
    ///
    /// If the package has a resolved reference URL, uses that.
    /// Otherwise, uses the fallback URL pattern.
    pub fn topic_url(&mut self, package: &str, topic: &str) -> Option<String> {
        if let Some(base_url) = self.resolve(package) {
            Some(format!("{}/{}.html", base_url.trim_end_matches('/'), topic))
        } else {
            self.options.fallback_url.as_ref().map(|pattern| {
                pattern
                    .replace("{package}", package)
                    .replace("{topic}", topic)
            })
        }
    }
}

/// Collect all external package references from an RdPackage
///
/// Scans all Rd files for `\link[pkg]{topic}` patterns and returns
/// the set of unique external package names.
pub fn collect_external_packages(package: &RdPackage) -> HashSet<String> {
    let mut packages = HashSet::new();

    for file in &package.files {
        if let Ok(content) = fs::read_to_string(file)
            && let Ok(doc) = rd2qmd_core::parse(&content)
        {
            for section in &doc.sections {
                collect_packages_from_nodes(&section.content, &mut packages);
            }
        }
    }

    packages
}

/// Recursively collect external package names from Rd nodes
fn collect_packages_from_nodes(nodes: &[RdNode], packages: &mut HashSet<String>) {
    for node in nodes {
        match node {
            RdNode::Link {
                package: Some(pkg),
                text,
                ..
            } => {
                // The parser stores "pkg:topic" as the package name for \link[pkg:topic]{text}
                // Extract just the package part (before the colon)
                let pkg_name = pkg.split(':').next().unwrap_or(pkg);
                packages.insert(pkg_name.to_string());
                // Also check text content
                if let Some(text_nodes) = text {
                    collect_packages_from_nodes(text_nodes, packages);
                }
            }
            RdNode::Link {
                text: Some(text), ..
            } => {
                collect_packages_from_nodes(text, packages);
            }
            // Recurse into container nodes with Vec<RdNode>
            RdNode::Code(children)
            | RdNode::Emph(children)
            | RdNode::Strong(children)
            | RdNode::Samp(children)
            | RdNode::SQuote(children)
            | RdNode::DQuote(children)
            | RdNode::Dfn(children)
            | RdNode::File(children)
            | RdNode::Kbd(children)
            | RdNode::Paragraph(children) => {
                collect_packages_from_nodes(children, packages);
            }
            RdNode::Href { text, .. } => {
                collect_packages_from_nodes(text, packages);
            }
            RdNode::Item { label, content } => {
                if let Some(label_nodes) = label {
                    collect_packages_from_nodes(label_nodes, packages);
                }
                collect_packages_from_nodes(content, packages);
            }
            RdNode::Itemize(items) | RdNode::Enumerate(items) => {
                collect_packages_from_nodes(items, packages);
            }
            RdNode::Describe(items) => {
                for item in items {
                    collect_packages_from_nodes(&item.term, packages);
                    collect_packages_from_nodes(&item.description, packages);
                }
            }
            RdNode::Tabular { rows, .. } => {
                for row in rows {
                    for cell in row {
                        collect_packages_from_nodes(cell, packages);
                    }
                }
            }
            RdNode::IfElse {
                then_content,
                else_content,
                ..
            } => {
                collect_packages_from_nodes(then_content, packages);
                collect_packages_from_nodes(else_content, packages);
            }
            RdNode::If { content, .. } => {
                collect_packages_from_nodes(content, packages);
            }
            RdNode::Section { title, content } | RdNode::Subsection { title, content } => {
                collect_packages_from_nodes(title, packages);
                collect_packages_from_nodes(content, packages);
            }
            RdNode::Macro { args, .. } => {
                for arg in args {
                    collect_packages_from_nodes(arg, packages);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_extract_reference_url_from_yaml() {
        let resolver = PackageUrlResolver::new(PackageUrlResolverOptions::default());

        // Test with urls.reference
        let yaml1 = r#"
url: https://dplyr.tidyverse.org
urls:
  reference: https://dplyr.tidyverse.org/reference
  article: https://dplyr.tidyverse.org/articles
"#;
        assert_eq!(
            resolver.extract_reference_url_from_yaml(yaml1),
            Some("https://dplyr.tidyverse.org/reference".to_string())
        );

        // Test with just url
        let yaml2 = r#"
url: https://dplyr.tidyverse.org
"#;
        assert_eq!(
            resolver.extract_reference_url_from_yaml(yaml2),
            Some("https://dplyr.tidyverse.org/reference".to_string())
        );

        // Test with trailing slash
        let yaml3 = r#"
url: https://dplyr.tidyverse.org/
"#;
        assert_eq!(
            resolver.extract_reference_url_from_yaml(yaml3),
            Some("https://dplyr.tidyverse.org/reference".to_string())
        );
    }

    #[test]
    fn test_collect_external_packages() {
        let dir = tempdir().unwrap();

        // Create a test Rd file with external links
        let rd_content = r#"\name{test}
\alias{test}
\title{Test}
\description{
See \code{\link[rlang:dyn-dots]{dynamic-dots}} and \link[dplyr]{mutate}.
Also \link[base]{paste} and \link{local_func}.
}
"#;
        fs::write(dir.path().join("test.Rd"), rd_content).unwrap();

        let package = RdPackage::from_directory(dir.path(), false).unwrap();
        let external = collect_external_packages(&package);

        // The parser stores "rlang:dyn-dots" as the package name for \link[rlang:dyn-dots]{...}
        // We extract just the package part (before the colon)
        assert!(
            external.contains("rlang"),
            "Expected 'rlang' in {:?}",
            external
        );
        assert!(external.contains("dplyr"));
        assert!(external.contains("base"));
        // local_func should not be included (no package specified)
        assert!(!external.contains("local_func"));
    }

    #[test]
    fn test_topic_url_with_fallback() {
        let mut resolver = PackageUrlResolver::new(PackageUrlResolverOptions {
            lib_paths: vec![],
            cache_dir: None,
            fallback_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
            enable_http: false,
        });

        // Package not found, should use fallback
        let url = resolver.topic_url("dplyr", "mutate");
        assert_eq!(
            url,
            Some("https://rdrr.io/pkg/dplyr/man/mutate.html".to_string())
        );
    }

    #[test]
    fn test_resolve_with_local_description() {
        let dir = tempdir().unwrap();
        let pkg_dir = dir.path().join("testpkg");
        fs::create_dir_all(&pkg_dir).unwrap();

        // Create a DESCRIPTION file
        let desc = r#"Package: testpkg
Title: Test Package
Version: 1.0.0
Description: A test package.
License: MIT
URL: https://testpkg.example.com
"#;
        fs::write(pkg_dir.join("DESCRIPTION"), desc).unwrap();

        let mut resolver = PackageUrlResolver::new(PackageUrlResolverOptions {
            lib_paths: vec![dir.path().to_path_buf()],
            cache_dir: None,
            fallback_url: None,
            enable_http: false, // Don't actually fetch
        });

        // Should construct URL from DESCRIPTION
        let url = resolver.resolve("testpkg");
        assert_eq!(
            url,
            Some("https://testpkg.example.com/reference".to_string())
        );
    }

    #[test]
    fn test_resolve_packages_with_fallback_for_uninstalled() {
        let mut resolver = PackageUrlResolver::new(PackageUrlResolverOptions {
            lib_paths: vec![], // No lib paths, so no packages can be resolved
            cache_dir: None,
            fallback_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
            enable_http: false,
        });

        let packages: HashSet<String> = ["dplyr", "ggplot2", "tidyr"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let result = resolver.resolve_packages(&packages);

        // All packages should have fallback URLs
        assert_eq!(result.urls.len(), 3);
        assert_eq!(
            result.urls.get("dplyr"),
            Some(&"https://rdrr.io/pkg/dplyr/man".to_string())
        );
        assert_eq!(
            result.urls.get("ggplot2"),
            Some(&"https://rdrr.io/pkg/ggplot2/man".to_string())
        );
        assert_eq!(
            result.urls.get("tidyr"),
            Some(&"https://rdrr.io/pkg/tidyr/man".to_string())
        );

        // All should be marked as NotInstalled fallbacks
        assert_eq!(result.fallbacks.len(), 3);
        assert_eq!(
            result.fallbacks.get("dplyr"),
            Some(&FallbackReason::NotInstalled)
        );
        assert_eq!(
            result.fallbacks.get("ggplot2"),
            Some(&FallbackReason::NotInstalled)
        );
        assert_eq!(
            result.fallbacks.get("tidyr"),
            Some(&FallbackReason::NotInstalled)
        );
    }

    #[test]
    fn test_resolve_packages_with_mixed_results() {
        let dir = tempdir().unwrap();
        let pkg_dir = dir.path().join("installed_pkg");
        fs::create_dir_all(&pkg_dir).unwrap();

        // Create a package WITHOUT pkgdown site (no URL in DESCRIPTION)
        let desc = r#"Package: installed_pkg
Title: Installed Package
Version: 1.0.0
Description: A test package without pkgdown.
License: MIT
"#;
        fs::write(pkg_dir.join("DESCRIPTION"), desc).unwrap();

        let mut resolver = PackageUrlResolver::new(PackageUrlResolverOptions {
            lib_paths: vec![dir.path().to_path_buf()],
            cache_dir: None,
            fallback_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
            enable_http: false,
        });

        let packages: HashSet<String> = ["installed_pkg", "uninstalled_pkg"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let result = resolver.resolve_packages(&packages);

        // Both should have URLs (fallback)
        assert_eq!(result.urls.len(), 2);

        // Check fallback reasons
        assert_eq!(result.fallbacks.len(), 2);
        assert_eq!(
            result.fallbacks.get("installed_pkg"),
            Some(&FallbackReason::NoPkgdownSite)
        );
        assert_eq!(
            result.fallbacks.get("uninstalled_pkg"),
            Some(&FallbackReason::NotInstalled)
        );
    }
}
