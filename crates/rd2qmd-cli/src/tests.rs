use crate::cli::{ConvertArgs, InputFormat, OutputFormat};
use crate::config::{self, ArgumentsFormat as CliArgumentsFormat, Config};
use crate::config_merge::{
    merge_arguments_format, merge_external_link_options, merge_external_link_url, merge_format,
    merge_frontmatter, merge_pagetitle, merge_unqualified_link_url,
};
use rd2qmd_core::ArgumentsFormat;
use std::path::PathBuf;

/// Create a default ConvertArgs for testing
fn default_convert_args() -> ConvertArgs {
    ConvertArgs {
        input: PathBuf::new(),
        output: None,
        format: OutputFormat::Qmd,
        jobs: None,
        recursive: false,
        frontmatter: true,
        no_frontmatter: false,
        no_pagetitle: false,
        quarto_code_blocks: None,
        external_link_url: None,
        no_external_link_url: false,
        unqualified_link_url: None,
        no_unqualified_link_url: false,
        r_lib_paths: vec![],
        cache_dir: None,
        no_external_links: false,
        exec_dontrun: false,
        no_exec_donttest: false,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        arguments_format: None,
        topic_index: None,
        config: None,
        no_config: false,
        input_format: InputFormat::Rd,
    }
}

#[test]
fn test_merge_format_no_config() {
    let cli = default_convert_args();
    let config = Config::default();
    assert_eq!(merge_format(&cli, &config), OutputFormat::Qmd);
}

#[test]
fn test_merge_format_config_overrides_default() {
    let cli = default_convert_args();
    let config = Config {
        output: config::OutputConfig {
            format: Some("md".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(merge_format(&cli, &config), OutputFormat::Md);
}

#[test]
fn test_merge_format_cli_overrides_config() {
    let mut cli = default_convert_args();
    cli.format = OutputFormat::Rmd;
    let config = Config {
        output: config::OutputConfig {
            format: Some("md".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    // CLI is not default (Qmd), so CLI wins
    assert_eq!(merge_format(&cli, &config), OutputFormat::Rmd);
}

#[test]
fn test_merge_frontmatter_no_config() {
    let cli = default_convert_args();
    let config = Config::default();
    assert!(merge_frontmatter(&cli, &config));
}

#[test]
fn test_merge_frontmatter_config_disables() {
    let cli = default_convert_args();
    let config = Config {
        output: config::OutputConfig {
            frontmatter: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(!merge_frontmatter(&cli, &config));
}

#[test]
fn test_merge_frontmatter_cli_no_frontmatter() {
    let mut cli = default_convert_args();
    cli.no_frontmatter = true;
    let config = Config {
        output: config::OutputConfig {
            frontmatter: Some(true),
            ..Default::default()
        },
        ..Default::default()
    };
    // --no-frontmatter should override config
    assert!(!merge_frontmatter(&cli, &config));
}

#[test]
fn test_merge_pagetitle_no_config() {
    let cli = default_convert_args();
    let config = Config::default();
    assert!(merge_pagetitle(&cli, &config));
}

#[test]
fn test_merge_pagetitle_config_disables() {
    let cli = default_convert_args();
    let config = Config {
        output: config::OutputConfig {
            pagetitle: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(!merge_pagetitle(&cli, &config));
}

#[test]
fn test_merge_pagetitle_cli_no_pagetitle() {
    let mut cli = default_convert_args();
    cli.no_pagetitle = true;
    let config = Config {
        output: config::OutputConfig {
            pagetitle: Some(true),
            ..Default::default()
        },
        ..Default::default()
    };
    // --no-pagetitle should override config
    assert!(!merge_pagetitle(&cli, &config));
}

#[test]
fn test_merge_unqualified_link_url_default() {
    let cli = default_convert_args();
    let config = Config::default();
    let url = merge_unqualified_link_url(&cli, &config);
    assert_eq!(url, Some("https://rdrr.io/r/base/{topic}.html".to_string()));
}

#[test]
fn test_merge_unqualified_link_url_config_overrides_default() {
    let cli = default_convert_args();
    let config = Config {
        links: config::LinksConfig {
            unqualified_link_url: Some("https://example.com/{topic}".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let url = merge_unqualified_link_url(&cli, &config);
    assert_eq!(url, Some("https://example.com/{topic}".to_string()));
}

#[test]
fn test_merge_unqualified_link_url_cli_overrides_config() {
    let mut cli = default_convert_args();
    cli.unqualified_link_url = Some("x-r-help:{topic}".to_string());
    let config = Config {
        links: config::LinksConfig {
            unqualified_link_url: Some("https://example.com/{topic}".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let url = merge_unqualified_link_url(&cli, &config);
    assert_eq!(url, Some("x-r-help:{topic}".to_string()));
}

#[test]
fn test_merge_unqualified_link_url_cli_disables() {
    let mut cli = default_convert_args();
    cli.no_unqualified_link_url = true;
    let config = Config {
        links: config::LinksConfig {
            unqualified_link_url: Some("https://example.com/{topic}".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    // --no-unqualified-link-url should disable
    assert_eq!(merge_unqualified_link_url(&cli, &config), None);
}

#[test]
fn test_merge_external_link_url_default() {
    let cli = default_convert_args();
    let config = Config::default();
    assert_eq!(
        merge_external_link_url(&cli, &config),
        Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string())
    );
}

#[test]
fn test_merge_external_link_url_config_overrides_default() {
    let cli = default_convert_args();
    let config = Config {
        links: config::LinksConfig {
            external_link_url: Some("x-r-help:{package}/{topic}".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        merge_external_link_url(&cli, &config),
        Some("x-r-help:{package}/{topic}".to_string())
    );
}

#[test]
fn test_merge_external_link_url_cli_overrides_config() {
    let mut cli = default_convert_args();
    cli.external_link_url = Some("app-help:{package}/{topic}".to_string());
    let config = Config {
        links: config::LinksConfig {
            external_link_url: Some("x-r-help:{package}/{topic}".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    // CLI is explicitly set, so CLI wins over config
    assert_eq!(
        merge_external_link_url(&cli, &config),
        Some("app-help:{package}/{topic}".to_string())
    );
}

#[test]
fn test_merge_external_link_url_cli_disables() {
    let mut cli = default_convert_args();
    cli.no_external_link_url = true;
    let config = Config {
        links: config::LinksConfig {
            external_link_url: Some("x-r-help:{package}/{topic}".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    // --no-external-link-url should disable
    assert_eq!(merge_external_link_url(&cli, &config), None);
}

#[test]
fn test_merge_arguments_format_no_config() {
    let cli = default_convert_args();
    let config = Config::default();
    assert_eq!(
        merge_arguments_format(&cli, &config),
        ArgumentsFormat::ListTable
    );
}

#[test]
fn test_merge_arguments_format_config_overrides() {
    let cli = default_convert_args();
    let config = Config {
        output: config::OutputConfig {
            arguments_format: Some(CliArgumentsFormat::PipeTable),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        merge_arguments_format(&cli, &config),
        ArgumentsFormat::PipeTable
    );
}

#[test]
fn test_merge_arguments_format_cli_overrides() {
    let mut cli = default_convert_args();
    cli.arguments_format = Some(CliArgumentsFormat::PipeTable);
    let config = Config {
        output: config::OutputConfig {
            arguments_format: Some(CliArgumentsFormat::GridTable),
            ..Default::default()
        },
        ..Default::default()
    };
    // CLI is explicitly set, so CLI wins over config
    assert_eq!(
        merge_arguments_format(&cli, &config),
        ArgumentsFormat::PipeTable
    );
}

#[test]
fn test_merge_arguments_format_list_table_cli_overrides_config() {
    let mut cli = default_convert_args();
    cli.arguments_format = Some(CliArgumentsFormat::ListTable);
    let config = Config {
        output: config::OutputConfig {
            arguments_format: Some(CliArgumentsFormat::GridTable),
            ..Default::default()
        },
        ..Default::default()
    };
    // Explicit --arguments-format list-table must override config grid-table
    assert_eq!(
        merge_arguments_format(&cli, &config),
        ArgumentsFormat::ListTable
    );
}

#[test]
fn test_merge_external_link_options_disabled_by_cli() {
    let mut cli = default_convert_args();
    cli.no_external_links = true;
    let config = Config {
        external: config::ExternalConfig {
            enabled: Some(true),
            lib_paths: Some(vec![std::path::PathBuf::from("/usr/lib/R")]),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(merge_external_link_options(&cli, &config).is_none());
}

#[test]
fn test_merge_external_link_options_disabled_by_config() {
    let cli = default_convert_args();
    let config = Config {
        external: config::ExternalConfig {
            enabled: Some(false),
            lib_paths: Some(vec![std::path::PathBuf::from("/usr/lib/R")]),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(merge_external_link_options(&cli, &config).is_none());
}

#[test]
fn test_merge_external_link_options_lib_paths_from_config() {
    let cli = default_convert_args();
    let config = Config {
        external: config::ExternalConfig {
            enabled: Some(true),
            lib_paths: Some(vec![std::path::PathBuf::from("/usr/lib/R")]),
            ..Default::default()
        },
        ..Default::default()
    };
    let opts = merge_external_link_options(&cli, &config).unwrap();
    assert_eq!(opts.lib_paths, vec![std::path::PathBuf::from("/usr/lib/R")]);
}

#[test]
fn test_merge_external_link_options_cli_overrides_lib_paths() {
    let mut cli = default_convert_args();
    cli.r_lib_paths = vec![std::path::PathBuf::from("/home/user/R")];
    let config = Config {
        external: config::ExternalConfig {
            enabled: Some(true),
            lib_paths: Some(vec![std::path::PathBuf::from("/usr/lib/R")]),
            ..Default::default()
        },
        ..Default::default()
    };
    let opts = merge_external_link_options(&cli, &config).unwrap();
    // CLI lib_paths should override config
    assert_eq!(
        opts.lib_paths,
        vec![std::path::PathBuf::from("/home/user/R")]
    );
}
