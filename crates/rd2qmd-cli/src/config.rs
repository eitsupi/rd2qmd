//! Configuration file support for rd2qmd CLI
//!
//! Loads settings from `_rd2qmd.toml` configuration file.

use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Default configuration file name (following Quarto's `_quarto.yml` convention)
pub const CONFIG_FILE_NAME: &str = "_rd2qmd.toml";

/// Schema URL for the configuration file
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/eitsupi/rd2qmd/main/crates/rd2qmd-cli/schema/rd2qmd.schema.json";

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
    /// Output format: "qmd" (Quarto Markdown), "md" (standard Markdown), or "rmd" (R Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Add YAML frontmatter with title (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<bool>,
    /// Add pkgdown-style pagetitle metadata ("<title> — <name>") (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagetitle: Option<bool>,
    /// Table format for Arguments section: "grid" (Pandoc grid table) or "pipe" (default: "grid")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments_table: Option<String>,
}

impl OutputConfig {
    fn is_empty(&self) -> bool {
        self.format.is_none()
            && self.frontmatter.is_none()
            && self.pagetitle.is_none()
            && self.arguments_table.is_none()
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
    /// URL pattern for unresolved links. Use {topic} as placeholder for the topic name.
    /// (default: "https://rdrr.io/r/base/{topic}.html")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved_url: Option<String>,
}

impl LinksConfig {
    fn is_empty(&self) -> bool {
        self.unresolved_url.is_none()
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
    /// Fallback URL pattern for packages without pkgdown sites.
    /// Use {package} and {topic} as placeholders.
    /// (default: "https://rdrr.io/pkg/{package}/man/{topic}.html")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_url: Option<String>,
}

impl ExternalConfig {
    fn is_empty(&self) -> bool {
        self.enabled.is_none()
            && self.lib_paths.is_none()
            && self.cache_dir.is_none()
            && self.fallback_url.is_none()
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
    pub fn json_schema() -> schemars::schema::RootSchema {
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
                format: Some("qmd".to_string()),
                frontmatter: Some(true),
                pagetitle: Some(true),
                arguments_table: Some("grid".to_string()),
            },
            code: CodeConfig {
                quarto_code_blocks: None, // auto-detect
                exec_dontrun: Some(false),
                exec_donttest: Some(true),
            },
            links: LinksConfig {
                unresolved_url: Some("https://rdrr.io/r/base/{topic}.html".to_string()),
            },
            external: ExternalConfig {
                enabled: Some(true),
                lib_paths: None, // user should specify
                cache_dir: None, // use system default
                fallback_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
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
            arguments_table = "pipe"
            "#,
        )
        .unwrap();

        assert_eq!(config.output.format, Some("md".to_string()));
        assert_eq!(config.output.frontmatter, Some(false));
        assert_eq!(config.output.pagetitle, Some(true));
        assert_eq!(config.output.arguments_table, Some("pipe".to_string()));
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
            unresolved_url = "https://example.com/{topic}.html"
            "#,
        )
        .unwrap();

        assert_eq!(
            config.links.unresolved_url,
            Some("https://example.com/{topic}.html".to_string())
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
            fallback_url = "https://rdrr.io/pkg/{package}/man/{topic}.html"
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
        assert_eq!(
            config.external.fallback_url,
            Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string())
        );
    }

    #[test]
    fn test_parse_full_config() {
        let config: Config = toml::from_str(
            r#"
            [output]
            format = "qmd"
            frontmatter = true
            pagetitle = true
            arguments_table = "grid"

            [code]
            quarto_code_blocks = true
            exec_dontrun = false
            exec_donttest = true

            [links]
            unresolved_url = "https://rdrr.io/r/base/{topic}.html"

            [external]
            enabled = true
            lib_paths = ["/usr/local/lib/R/site-library"]
            cache_dir = "/tmp/rd2qmd-cache"
            fallback_url = "https://rdrr.io/pkg/{package}/man/{topic}.html"
            "#,
        )
        .unwrap();

        assert_eq!(config.output.format, Some("qmd".to_string()));
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

        assert_eq!(config.output.format, Some("md".to_string()));
        // Other sections should be default
        assert!(config.code.quarto_code_blocks.is_none());
        assert!(config.links.unresolved_url.is_none());
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
