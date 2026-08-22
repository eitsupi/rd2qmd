//! Configuration file support for rd2qmd CLI
//!
//! Loads settings from `_rd2qmd.toml` configuration file.

use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::cli::OutputFormat;

/// Output format for the Arguments section
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "kebab-case")]
pub enum ArgumentsFormat {
    /// Pipe table - limited to inline content
    PipeTable,
    /// Pandoc grid table - supports block elements (lists, paragraphs) in cells
    GridTable,
    /// Quarto list-table (default) - requires Quarto 1.9+, compatible with q2
    #[default]
    ListTable,
    /// Markdown loose list - bold inline code name + indented description; compatible everywhere
    List,
}

/// Default configuration file name (following Quarto's `_quarto.yml` convention)
pub const CONFIG_FILE_NAME: &str = "_rd2qmd.toml";

/// Schema URL for the configuration file
pub const SCHEMA_URL: &str = "https://raw.githubusercontent.com/eitsupi/rd2qmd/main/crates/rd2qmd-cli/schema/rd2qmd.schema.json";

/// Root configuration structure
#[derive(Debug, Default, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct Config {
    /// Output format configuration
    #[serde(skip_serializing_if = "OutputConfig::is_empty")]
    pub output: OutputConfig,
    /// Code block configuration
    #[serde(skip_serializing_if = "CodeConfig::is_empty")]
    pub code: CodeConfig,
    /// Link resolution configuration
    #[serde(skip_serializing_if = "LinksConfig::is_empty")]
    pub links: LinksConfig,
    /// External package link resolution configuration
    #[serde(skip_serializing_if = "ExternalConfig::is_empty")]
    pub external: ExternalConfig,
}

/// Output format configuration
#[derive(Debug, Default, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct OutputConfig {
    /// Output format: "qmd" (Quarto Markdown), "md" (standard Markdown), "rmd" (R Markdown), or "typ" (Typst)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    /// Add YAML frontmatter with title (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<bool>,
    /// Add pkgdown-style pagetitle metadata ("<title> — <name>") (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagetitle: Option<bool>,
    /// Output format for Arguments section (default: list-table)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments_format: Option<ArgumentsFormat>,
    /// Include topics with \keyword{internal} (default: false)
    /// By default, internal topics are skipped (matching pkgdown behavior).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_internal: Option<bool>,
    /// Prefer the ASCII representation of \eqn{}/\deqn{} equations over LaTeX
    /// math when one is present (default: false). Intended for renderers
    /// without math support, such as terminal pagers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefer_ascii_math: Option<bool>,
}

impl OutputConfig {
    fn is_empty(&self) -> bool {
        self.format.is_none()
            && self.frontmatter.is_none()
            && self.pagetitle.is_none()
            && self.arguments_format.is_none()
            && self.include_internal.is_none()
            && self.prefer_ascii_math.is_none()
    }
}

/// Code block configuration
#[derive(Debug, Default, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct CodeConfig {
    /// Use Quarto {r} code blocks instead of plain r blocks (auto-set based on format if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarto_code_blocks: Option<bool>,
    #[doc = r"Make \dontrun{} example code executable ({r} blocks) (default: false)"]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_dontrun: Option<bool>,
    #[doc = r"Make \donttest{} example code executable (default: true)"]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_donttest: Option<bool>,
}

impl CodeConfig {
    fn is_empty(&self) -> bool {
        self.quarto_code_blocks.is_none()
            && self.exec_dontrun.is_none()
            && self.exec_donttest.is_none()
    }
}

/// Link resolution configuration
#[derive(Debug, Default, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct LinksConfig {
    /// URL template for qualified links (`\link[pkg]{topic}`) whose package is
    /// not found in `package_urls`. Use {package} and {topic} as placeholders.
    /// (default: "https://rdrr.io/pkg/{package}/man/{topic}.html")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_link_url: Option<String>,
    /// URL template for unqualified links (`\link{topic}`) when alias lookup fails.
    /// Use {topic} as placeholder.
    /// (default: "https://rdrr.io/r/base/{topic}.html")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unqualified_link_url: Option<String>,
    /// URL template for internal links resolved via the alias index.
    /// Use {file} for the alias-resolved file basename and {topic} for the
    /// link topic. (default: derived from the output format as "{file}.<ext>")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_link_url: Option<String>,
    /// Package URL map: package name -> full URL template with a {topic}
    /// placeholder. Entries take precedence over automatic external link
    /// resolution and over `external_link_url`.
    /// Example: `dplyr = "https://dplyr.tidyverse.org/reference/{topic}.html"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_urls: Option<HashMap<String, String>>,
}

impl LinksConfig {
    fn is_empty(&self) -> bool {
        self.external_link_url.is_none()
            && self.unqualified_link_url.is_none()
            && self.internal_link_url.is_none()
            && self.package_urls.is_none()
    }
}

/// External package link resolution configuration
#[derive(Debug, Default, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct ExternalConfig {
    /// Enable external package link resolution (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// R library paths to search for external packages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lib_paths: Option<Vec<PathBuf>>,
    /// Cache directory for pkgdown.yml files (default: system temp directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<PathBuf>,
}

impl ExternalConfig {
    fn is_empty(&self) -> bool {
        self.enabled.is_none() && self.lib_paths.is_none() && self.cache_dir.is_none()
    }
}

impl Config {
    /// Load configuration from a specific file path
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }

    /// Try to load configuration from a directory (looks for `_rd2qmd.toml`)
    ///
    /// Returns `Ok(None)` if the config file doesn't exist.
    pub fn load_from_dir(dir: &Path) -> Result<Option<Self>> {
        let config_path = dir.join(CONFIG_FILE_NAME);
        if config_path.exists() {
            Ok(Some(Self::load(&config_path)?))
        } else {
            Ok(None)
        }
    }

    /// Generate JSON schema for the configuration
    pub fn json_schema() -> schemars::Schema {
        schemars::schema_for!(Config)
    }

    /// Generate JSON schema as a string
    pub fn json_schema_string() -> Result<String> {
        let schema = Self::json_schema();
        serde_json::to_string_pretty(&schema).context("Failed to serialize JSON schema")
    }

    /// Serialize configuration to TOML string with schema directive
    pub fn to_toml_with_schema(&self) -> Result<String> {
        let toml_content =
            toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;

        Ok(format!("#:schema {}\n\n{}", SCHEMA_URL, toml_content))
    }

    /// Create a sample configuration with common defaults for init command
    pub fn sample() -> Self {
        Config {
            output: OutputConfig {
                format: Some(OutputFormat::Qmd),
                frontmatter: Some(true),
                pagetitle: Some(true),
                arguments_format: Some(ArgumentsFormat::ListTable),
                include_internal: Some(false),
                prefer_ascii_math: None, // enable for renderers without math support
            },
            code: CodeConfig {
                quarto_code_blocks: None, // auto-detect
                exec_dontrun: Some(false),
                exec_donttest: Some(true),
            },
            links: LinksConfig {
                external_link_url: Some(
                    "https://rdrr.io/pkg/{package}/man/{topic}.html".to_string(),
                ),
                unqualified_link_url: Some("https://rdrr.io/r/base/{topic}.html".to_string()),
                internal_link_url: None, // derived from the output format
                package_urls: None,      // user should specify
            },
            external: ExternalConfig {
                enabled: Some(true),
                lib_paths: None, // user should specify
                cache_dir: None, // use system default
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_config() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.output.format.is_none());
        assert!(config.output.frontmatter.is_none());
    }

    #[test]
    fn test_parse_output_section() {
        let config: Config = toml::from_str(
            r#"
            [output]
            format = "md"
            frontmatter = false
            pagetitle = true
            arguments_format = "pipe-table"
            "#,
        )
        .unwrap();

        assert_eq!(config.output.format, Some(OutputFormat::Md));
        assert_eq!(config.output.frontmatter, Some(false));
        assert_eq!(config.output.pagetitle, Some(true));
        assert_eq!(
            config.output.arguments_format,
            Some(ArgumentsFormat::PipeTable)
        );
    }

    #[test]
    fn test_output_format_rejects_typos_and_accepts_typst_alias() {
        let error = toml::from_str::<Config>("[output]\nformat = \"typs\"\n")
            .expect_err("an unknown output format must be rejected");
        assert!(error.to_string().contains("unknown variant `typs`"));

        let config: Config = toml::from_str("[output]\nformat = \"typst\"\n").unwrap();
        assert_eq!(config.output.format, Some(OutputFormat::Typ));
    }

    #[test]
    fn test_parse_code_section() {
        let config: Config = toml::from_str(
            r#"
            [code]
            quarto_code_blocks = true
            exec_dontrun = false
            exec_donttest = true
            "#,
        )
        .unwrap();

        assert_eq!(config.code.quarto_code_blocks, Some(true));
        assert_eq!(config.code.exec_dontrun, Some(false));
        assert_eq!(config.code.exec_donttest, Some(true));
    }

    #[test]
    fn test_parse_links_section() {
        let config: Config = toml::from_str(
            r#"
            [links]
            unqualified_link_url = "https://example.com/{topic}.html"
            external_link_url = "x-r-help:{package}/{topic}"
            internal_link_url = "/reference/{file}.html"
            "#,
        )
        .unwrap();

        assert_eq!(
            config.links.unqualified_link_url,
            Some("https://example.com/{topic}.html".to_string())
        );
        assert_eq!(
            config.links.external_link_url,
            Some("x-r-help:{package}/{topic}".to_string())
        );
        assert_eq!(
            config.links.internal_link_url,
            Some("/reference/{file}.html".to_string())
        );
        assert!(config.links.package_urls.is_none());
    }

    #[test]
    fn test_parse_links_package_urls() {
        let config: Config = toml::from_str(
            r#"
            [links.package_urls]
            dplyr = "https://dplyr.tidyverse.org/reference/{topic}.html"
            rlang = "https://rlang.r-lib.org/reference/{topic}.html"
            "#,
        )
        .unwrap();

        let package_urls = config.links.package_urls.unwrap();
        assert_eq!(package_urls.len(), 2);
        assert_eq!(
            package_urls.get("dplyr"),
            Some(&"https://dplyr.tidyverse.org/reference/{topic}.html".to_string())
        );
        assert_eq!(
            package_urls.get("rlang"),
            Some(&"https://rlang.r-lib.org/reference/{topic}.html".to_string())
        );
    }

    #[test]
    fn test_parse_external_section() {
        let config: Config = toml::from_str(
            r#"
            [external]
            enabled = true
            lib_paths = ["/usr/lib/R", "/home/user/R"]
            cache_dir = "/tmp/cache"
            "#,
        )
        .unwrap();

        assert_eq!(config.external.enabled, Some(true));
        assert_eq!(
            config.external.lib_paths,
            Some(vec![
                PathBuf::from("/usr/lib/R"),
                PathBuf::from("/home/user/R")
            ])
        );
        assert_eq!(config.external.cache_dir, Some(PathBuf::from("/tmp/cache")));
    }

    #[test]
    fn test_parse_full_config() {
        let config: Config = toml::from_str(
            r#"
            [output]
            format = "qmd"
            frontmatter = true
            pagetitle = true
            arguments_format = "grid-table"

            [code]
            quarto_code_blocks = true
            exec_dontrun = false
            exec_donttest = true

            [links]
            unqualified_link_url = "https://rdrr.io/r/base/{topic}.html"
            external_link_url = "https://rdrr.io/pkg/{package}/man/{topic}.html"

            [external]
            enabled = true
            lib_paths = ["/usr/local/lib/R/site-library"]
            cache_dir = "/tmp/rd2qmd-cache"
            "#,
        )
        .unwrap();

        assert_eq!(config.output.format, Some(OutputFormat::Qmd));
        assert_eq!(config.external.enabled, Some(true));
    }

    #[test]
    fn test_partial_config() {
        // Only some sections specified
        let config: Config = toml::from_str(
            r#"
            [output]
            format = "md"
            "#,
        )
        .unwrap();

        assert_eq!(config.output.format, Some(OutputFormat::Md));
        // Other sections should be default
        assert!(config.code.quarto_code_blocks.is_none());
        assert!(config.links.unqualified_link_url.is_none());
        assert!(config.external.enabled.is_none());
    }

    #[test]
    fn test_serialize_empty_config() {
        let config = Config::default();
        let toml = config.to_toml_with_schema().unwrap();
        assert!(toml.starts_with("#:schema"));
        // Empty config should have minimal content
        assert!(!toml.contains("[output]"));
    }

    #[test]
    fn test_serialize_sample_config() {
        let config = Config::sample();
        let toml = config.to_toml_with_schema().unwrap();
        assert!(toml.starts_with("#:schema"));
        assert!(toml.contains("[output]"));
        assert!(toml.contains("format = \"qmd\""));
    }

    #[test]
    fn test_json_schema_generation() {
        let schema = Config::json_schema_string().unwrap();
        assert!(schema.contains("\"title\""));
        assert!(schema.contains("OutputConfig"));
    }

    #[test]
    fn test_roundtrip() {
        let config = Config::sample();
        let toml = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml).unwrap();
        assert_eq!(config.output.format, parsed.output.format);
    }
}
