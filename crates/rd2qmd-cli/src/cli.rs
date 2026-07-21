//! Clap CLI surface: argument/subcommand definitions.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::config::ArgumentsFormat as CliArgumentsFormat;
use rd2qmd_package::InputFormat as PackageInputFormat;

/// Options for external package link resolution
#[derive(Debug, Clone)]
pub(crate) struct ExternalLinkOptions {
    pub(crate) lib_paths: Vec<PathBuf>,
    pub(crate) cache_dir: Option<PathBuf>,
}

/// Output format for markdown conversion
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    /// Quarto Markdown (.qmd) - uses {r} code blocks for examples
    #[default]
    Qmd,
    /// Standard Markdown (.md) - uses plain r code blocks
    Md,
    /// R Markdown (.Rmd) - uses {r} code blocks for examples
    Rmd,
}

/// Input format for Rd documentation
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum InputFormat {
    /// Rd source files (.Rd)
    #[default]
    Rd,
    /// Pre-parsed AST JSON envelopes produced by `rd2qmd parse` (.json)
    Ast,
}

impl InputFormat {
    pub(crate) fn file_extension(self) -> &'static str {
        match self {
            Self::Rd => ".Rd",
            Self::Ast => ".json",
        }
    }
}

impl From<InputFormat> for PackageInputFormat {
    fn from(format: InputFormat) -> Self {
        match format {
            InputFormat::Rd => PackageInputFormat::Rd,
            InputFormat::Ast => PackageInputFormat::AstJson,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "rd2qmd")]
#[command(about = "Convert Rd files to Quarto Markdown")]
#[command(version)]
#[command(arg_required_else_help = true)]
#[command(after_help = "Examples:
  rd2qmd convert file.Rd                    # Convert single file to file.qmd
  rd2qmd convert file.Rd -o output.qmd      # Convert to specific output file
  rd2qmd convert file.Rd -f md              # Convert to standard Markdown (.md)
  rd2qmd convert file.Rd -f rmd             # Convert to R Markdown (.Rmd)
  rd2qmd convert man/ -o docs/              # Convert directory (with alias resolution)
  rd2qmd convert man/ -o docs/ -j4          # Use 4 parallel jobs
  rd2qmd convert man/ --topic-index i.json  # Convert and generate topic index
  rd2qmd parse man/ -o ast_dir/             # Parse to AST JSON for later convert
  rd2qmd convert ast_dir/ --input-format ast -o docs/  # Convert AST JSON back
  rd2qmd index man/                 # Generate topic index JSON to stdout
  rd2qmd index man/ | jq '.topics[] | select(.lifecycle)'")]
pub(crate) struct Cli {
    /// Subcommand
    #[command(subcommand)]
    pub(crate) subcommand: Commands,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub(crate) verbose: bool,

    /// Quiet mode - only show errors
    #[arg(short, long, global = true)]
    pub(crate) quiet: bool,
}

/// Arguments for the convert subcommand
#[derive(Args, Debug)]
pub(crate) struct ConvertArgs {
    /// Input Rd file or directory
    pub(crate) input: PathBuf,

    /// Output file or directory
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,

    /// Output format: qmd (Quarto) or md (standard Markdown)
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Qmd)]
    pub(crate) format: OutputFormat,

    /// Number of parallel jobs (defaults to number of CPUs)
    #[arg(short, long)]
    pub(crate) jobs: Option<usize>,

    /// Process directories recursively
    #[arg(short, long)]
    pub(crate) recursive: bool,

    /// Add YAML frontmatter with title
    #[arg(long, default_value = "true")]
    pub(crate) frontmatter: bool,

    /// Disable YAML frontmatter
    #[arg(long, conflicts_with = "frontmatter")]
    pub(crate) no_frontmatter: bool,

    /// Skip pkgdown-style pagetitle metadata ("<title> — <name>")
    #[arg(long)]
    pub(crate) no_pagetitle: bool,

    /// Use Quarto {r} code blocks instead of r (auto-set based on format)
    #[arg(long)]
    pub(crate) quarto_code_blocks: Option<bool>,

    /// URL template for qualified links (\link[pkg]{topic}) whose target
    /// package has no known documentation URL. Rd links are either qualified
    /// (the package is named) or unqualified (topic only); qualified links
    /// resolve through the package_urls map (config [links.package_urls],
    /// merged with automatic pkgdown-site resolution in directory mode)
    /// first, and this template is the last-resort fallback before the link
    /// degrades to plain inline code. Use {package} and {topic} as
    /// placeholders. Two typical uses: point links at an aggregator site
    /// that hosts documentation for all packages (the default,
    /// https://rdrr.io/pkg/{package}/man/{topic}.html), or emit a custom URI
    /// scheme such as x-r-help:{package}/{topic} for viewers that resolve
    /// help topics themselves (e.g. a terminal help browser).
    /// [default: https://rdrr.io/pkg/{package}/man/{topic}.html]
    #[arg(long, value_name = "TEMPLATE")]
    pub(crate) external_link_url: Option<String>,

    /// Disable the qualified-link fallback template; qualified links whose
    /// package has no known documentation URL become plain inline code
    #[arg(long, conflicts_with = "external_link_url")]
    pub(crate) no_external_link_url: bool,

    /// URL template for unqualified links (\link{topic}) that alias
    /// resolution cannot resolve. In directory mode, unqualified links are
    /// first resolved against the package's own alias index (producing
    /// internal links); this template is the last-resort fallback for topics
    /// not found there (typically base R topics) before the link degrades to
    /// plain inline code. Use {topic} as placeholder. Point it at an
    /// aggregator site (the default, https://rdrr.io/r/base/{topic}.html),
    /// or emit a custom URI scheme such as x-r-help:{topic} for viewers that
    /// resolve help topics themselves.
    /// [default: https://rdrr.io/r/base/{topic}.html]
    #[arg(long, value_name = "TEMPLATE")]
    pub(crate) unqualified_link_url: Option<String>,

    /// Disable the unqualified-link fallback template; unqualified links
    /// that alias resolution cannot resolve become plain inline code
    #[arg(long, conflicts_with = "unqualified_link_url")]
    pub(crate) no_unqualified_link_url: bool,

    /// R library path to search for external packages (can be specified multiple times)
    #[arg(long = "r-lib-path", value_name = "PATH")]
    pub(crate) r_lib_paths: Vec<PathBuf>,

    /// Cache directory for pkgdown.yml files (default: system temp directory)
    #[arg(long, value_name = "DIR")]
    pub(crate) cache_dir: Option<PathBuf>,

    /// Disable external package link resolution
    #[arg(long)]
    pub(crate) no_external_links: bool,

    /// Make \dontrun{} example code executable ({r} blocks)
    #[arg(long)]
    pub(crate) exec_dontrun: bool,

    /// Don't make \donttest{} example code executable (by default it is executable)
    #[arg(long)]
    pub(crate) no_exec_donttest: bool,

    /// Include topics with \keyword{internal} in the output
    /// By default, internal topics are skipped (matching pkgdown behavior).
    #[arg(long)]
    pub(crate) include_internal: bool,

    /// Include \if{html}{...} blocks in the output
    /// By default, HTML-conditional content is excluded because it targets HTML
    /// renderers (CRAN HTML manual, pkgdown, etc.) and often contains raw HTML
    /// markup that produces noise in plain Markdown. Enable when targeting an
    /// HTML-capable renderer such as Quarto HTML output.
    #[arg(long)]
    pub(crate) include_html_output: bool,

    /// Prefer the ASCII representation of \eqn{}/\deqn{} equations over LaTeX
    /// math when one is present. Intended for renderers without math support,
    /// such as terminal pagers: \eqn becomes inline code and \deqn becomes a
    /// plain code block.
    #[arg(long)]
    pub(crate) prefer_ascii_math: bool,

    /// Output format for the Arguments section: list-table (Quarto list-table, default), grid-table
    /// (Pandoc grid table), pipe-table (GFM pipe table, inline only), or list (Markdown loose list).
    #[arg(long, value_enum)]
    pub(crate) arguments_format: Option<CliArgumentsFormat>,

    /// Generate topic index JSON file (directory mode only)
    /// Contains topic names, files, titles, aliases, and lifecycle stages
    #[arg(long, value_name = "FILE")]
    pub(crate) topic_index: Option<PathBuf>,

    /// Path to configuration file (default: _rd2qmd.toml in current directory)
    #[arg(long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,

    /// Ignore configuration file
    #[arg(long)]
    pub(crate) no_config: bool,

    /// Input format: rd (default) or ast (pre-parsed AST JSON from `rd2qmd parse`)
    /// A single-file `.json` input is auto-detected as ast without this flag.
    #[arg(long, value_enum, default_value_t = InputFormat::Rd)]
    pub(crate) input_format: InputFormat,
}

/// Subcommands
#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Convert Rd files to Quarto Markdown (or standard Markdown / R Markdown)
    Convert(Box<ConvertArgs>),

    /// Parse Rd files to AST JSON
    ///
    /// Parses Rd files (single file or directory) into a versioned JSON
    /// envelope around the parsed document, letting external tooling inspect
    /// or rewrite it before a later `convert` run turns it into Markdown.
    /// Has no conversion options and does not read `_rd2qmd.toml`.
    Parse(ParseArgs),

    /// Generate topic index JSON to stdout
    ///
    /// Parses all Rd files in the directory and outputs a JSON index
    /// containing topic metadata (name, file, title, aliases, lifecycle).
    /// Use with jq for filtering: rd2qmd index man/ | jq '.topics[]'
    Index(IndexArgs),

    /// Initialize a configuration file (_rd2qmd.toml)
    ///
    /// Creates a new configuration file with all options commented out.
    /// Includes schema directive for editor support (tombi, taplo, etc.)
    Init(InitArgs),
}

/// Arguments for the index subcommand
#[derive(Args, Debug)]
pub(crate) struct IndexArgs {
    /// Input directory containing Rd files
    pub(crate) input: PathBuf,

    /// Output format extension (used for file field in JSON)
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Qmd)]
    pub(crate) format: OutputFormat,

    /// Process directories recursively
    #[arg(short, long)]
    pub(crate) recursive: bool,

    /// Include topics with \keyword{internal} in the index
    /// By default, internal topics are excluded (matching pkgdown behavior).
    #[arg(long)]
    pub(crate) include_internal: bool,

    /// Input format: rd (default) or ast (pre-parsed AST JSON from `rd2qmd parse`)
    #[arg(long, value_enum, default_value_t = InputFormat::Rd)]
    pub(crate) input_format: InputFormat,
}

/// Arguments for the parse subcommand
#[derive(Args, Debug)]
pub(crate) struct ParseArgs {
    /// Input Rd file or directory
    pub(crate) input: PathBuf,

    /// Output file or directory (default: input stem + .json, or the input
    /// directory itself in directory mode)
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,

    /// Process directories recursively
    #[arg(short, long)]
    pub(crate) recursive: bool,

    /// Number of parallel jobs (defaults to number of CPUs)
    #[arg(short, long)]
    pub(crate) jobs: Option<usize>,
}

/// Arguments for the init subcommand
#[derive(Args, Debug)]
pub(crate) struct InitArgs {
    /// Output path for configuration file (default: _rd2qmd.toml)
    #[arg(short, long, default_value = "_rd2qmd.toml")]
    pub(crate) output: PathBuf,

    /// Overwrite existing file
    #[arg(long)]
    pub(crate) force: bool,

    /// Output JSON schema to stdout instead of creating config file
    #[arg(long)]
    pub(crate) schema: bool,
}
