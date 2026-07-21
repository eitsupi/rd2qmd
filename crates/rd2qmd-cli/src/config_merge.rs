//! Reconciliation of CLI-flag overrides with values loaded from `config.rs`'s
//! `Config` type: CLI > Config > Default precedence for each convert option.

use anyhow::{Context, Result};

use crate::cli::{ConvertArgs, ExternalLinkOptions, OutputFormat};
use crate::config::{ArgumentsFormat as CliArgumentsFormat, Config};
use rd2qmd_core::ArgumentsFormat;

/// Default URL template for qualified links (`\link[pkg]{topic}`) whose
/// package has no known documentation URL
const DEFAULT_EXTERNAL_LINK_URL: &str = "https://rdrr.io/pkg/{package}/man/{topic}.html";

/// Default URL template for unqualified links (`\link{topic}`) that alias
/// resolution cannot resolve
const DEFAULT_UNQUALIFIED_LINK_URL: &str = "https://rdrr.io/r/base/{topic}.html";

/// Load configuration file based on CLI options
pub(crate) fn load_config(args: &ConvertArgs) -> Result<Config> {
    if args.no_config {
        return Ok(Config::default());
    }

    if let Some(path) = &args.config {
        return Config::load(path);
    }

    // Try to load from current directory
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    Ok(Config::load_from_dir(&cwd)?.unwrap_or_default())
}

/// Merge format: CLI default vs config
///
/// Since clap has a default value, we can't tell if the user explicitly set it.
/// We use a heuristic: if config specifies a format, use it unless CLI is non-default.
/// This means config wins when CLI uses default (qmd).
pub(crate) fn merge_format(args: &ConvertArgs, config: &Config) -> OutputFormat {
    // If config specifies a format, check if CLI is using the default
    if let Some(ref fmt) = config.output.format {
        // Only use config if CLI appears to be using default
        // This is a heuristic - we assume if CLI is Qmd (default), config should win
        if args.format == OutputFormat::Qmd {
            return match fmt.to_lowercase().as_str() {
                "md" => OutputFormat::Md,
                "rmd" => OutputFormat::Rmd,
                _ => OutputFormat::Qmd,
            };
        }
    }
    args.format
}

/// Merge frontmatter setting
pub(crate) fn merge_frontmatter(args: &ConvertArgs, config: &Config) -> bool {
    // CLI --no-frontmatter explicitly disables
    if args.no_frontmatter {
        return false;
    }
    // Config value if specified, otherwise CLI default (true)
    config.output.frontmatter.unwrap_or(args.frontmatter)
}

/// Merge pagetitle setting
pub(crate) fn merge_pagetitle(args: &ConvertArgs, config: &Config) -> bool {
    // CLI --no-pagetitle explicitly disables
    if args.no_pagetitle {
        return false;
    }
    // Config value if specified, otherwise default (true)
    config.output.pagetitle.unwrap_or(true)
}

/// Merge unqualified link URL: CLI > Config > Default
pub(crate) fn merge_unqualified_link_url(args: &ConvertArgs, config: &Config) -> Option<String> {
    // CLI --no-unqualified-link-url explicitly disables
    if args.no_unqualified_link_url {
        return None;
    }
    args.unqualified_link_url
        .clone()
        .or_else(|| config.links.unqualified_link_url.clone())
        .or_else(|| Some(DEFAULT_UNQUALIFIED_LINK_URL.to_string()))
}

/// Merge external link URL: CLI > Config > Default
pub(crate) fn merge_external_link_url(args: &ConvertArgs, config: &Config) -> Option<String> {
    // CLI --no-external-link-url explicitly disables
    if args.no_external_link_url {
        return None;
    }
    args.external_link_url
        .clone()
        .or_else(|| config.links.external_link_url.clone())
        .or_else(|| Some(DEFAULT_EXTERNAL_LINK_URL.to_string()))
}

/// Merge arguments format: explicit CLI > config > default (list-table)
pub(crate) fn merge_arguments_format(args: &ConvertArgs, config: &Config) -> ArgumentsFormat {
    let fmt = args
        .arguments_format
        .or(config.output.arguments_format)
        .unwrap_or_default();
    match fmt {
        CliArgumentsFormat::PipeTable => ArgumentsFormat::PipeTable,
        CliArgumentsFormat::GridTable => ArgumentsFormat::GridTable,
        CliArgumentsFormat::ListTable => ArgumentsFormat::ListTable,
        CliArgumentsFormat::List => ArgumentsFormat::List,
    }
}

/// Merge external link options
pub(crate) fn merge_external_link_options(
    args: &ConvertArgs,
    config: &Config,
) -> Option<ExternalLinkOptions> {
    // CLI --no-external-links explicitly disables
    if args.no_external_links {
        return None;
    }

    // Config can disable external links
    if let Some(false) = config.external.enabled {
        return None;
    }

    // Merge lib_paths: CLI takes precedence if specified
    let lib_paths = if !args.r_lib_paths.is_empty() {
        args.r_lib_paths.clone()
    } else {
        config.external.lib_paths.clone().unwrap_or_default()
    };

    // Merge cache_dir: CLI takes precedence
    let cache_dir = args.cache_dir.clone().or(config.external.cache_dir.clone());

    Some(ExternalLinkOptions {
        lib_paths,
        cache_dir,
    })
}
