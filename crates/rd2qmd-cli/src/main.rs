//! rd2qmd: CLI tool to convert Rd files to Quarto Markdown

mod config;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use config::{ArgumentsFormat as CliArgumentsFormat, Config};
use std::fs;
use std::path::{Path, PathBuf};

use rd2qmd_core::{ArgumentsFormat, RdConverter};
use rd2qmd_package::{
    ExternalLinkOptions as PackageExternalLinkOptions, FallbackReason, FullConvertResult,
    PackageConvertOptions, PackageConverter, RdPackage, TopicIndexOptions, generate_topic_index,
};

/// Default URL template for qualified links (`\link[pkg]{topic}`) whose
/// package has no known documentation URL
const DEFAULT_EXTERNAL_LINK_URL: &str = "https://rdrr.io/pkg/{package}/man/{topic}.html";

/// Default URL template for unqualified links (`\link{topic}`) that alias
/// resolution cannot resolve
const DEFAULT_UNQUALIFIED_LINK_URL: &str = "https://rdrr.io/r/base/{topic}.html";

/// Options for external package link resolution
#[derive(Debug, Clone)]
struct ExternalLinkOptions {
    lib_paths: Vec<PathBuf>,
    cache_dir: Option<PathBuf>,
}

/// Output format for markdown conversion
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
enum OutputFormat {
    /// Quarto Markdown (.qmd) - uses {r} code blocks for examples
    #[default]
    Qmd,
    /// Standard Markdown (.md) - uses plain r code blocks
    Md,
    /// R Markdown (.Rmd) - uses {r} code blocks for examples
    Rmd,
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
  rd2qmd index man/                 # Generate topic index JSON to stdout
  rd2qmd index man/ | jq '.topics[] | select(.lifecycle)'")]
struct Cli {
    /// Subcommand
    #[command(subcommand)]
    subcommand: Commands,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Quiet mode - only show errors
    #[arg(short, long, global = true)]
    quiet: bool,
}

/// Arguments for the convert subcommand
#[derive(Args, Debug)]
struct ConvertArgs {
    /// Input Rd file or directory
    input: PathBuf,

    /// Output file or directory
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format: qmd (Quarto) or md (standard Markdown)
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Qmd)]
    format: OutputFormat,

    /// Number of parallel jobs (defaults to number of CPUs)
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Process directories recursively
    #[arg(short, long)]
    recursive: bool,

    /// Add YAML frontmatter with title
    #[arg(long, default_value = "true")]
    frontmatter: bool,

    /// Disable YAML frontmatter
    #[arg(long, conflicts_with = "frontmatter")]
    no_frontmatter: bool,

    /// Skip pkgdown-style pagetitle metadata ("<title> — <name>")
    #[arg(long)]
    no_pagetitle: bool,

    /// Use Quarto {r} code blocks instead of r (auto-set based on format)
    #[arg(long)]
    quarto_code_blocks: Option<bool>,

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
    external_link_url: Option<String>,

    /// Disable the qualified-link fallback template; qualified links whose
    /// package has no known documentation URL become plain inline code
    #[arg(long, conflicts_with = "external_link_url")]
    no_external_link_url: bool,

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
    unqualified_link_url: Option<String>,

    /// Disable the unqualified-link fallback template; unqualified links
    /// that alias resolution cannot resolve become plain inline code
    #[arg(long, conflicts_with = "unqualified_link_url")]
    no_unqualified_link_url: bool,

    /// R library path to search for external packages (can be specified multiple times)
    #[arg(long = "r-lib-path", value_name = "PATH")]
    r_lib_paths: Vec<PathBuf>,

    /// Cache directory for pkgdown.yml files (default: system temp directory)
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,

    /// Disable external package link resolution
    #[arg(long)]
    no_external_links: bool,

    /// Make \dontrun{} example code executable ({r} blocks)
    #[arg(long)]
    exec_dontrun: bool,

    /// Don't make \donttest{} example code executable (by default it is executable)
    #[arg(long)]
    no_exec_donttest: bool,

    /// Include topics with \keyword{internal} in the output
    /// By default, internal topics are skipped (matching pkgdown behavior).
    #[arg(long)]
    include_internal: bool,

    /// Include \if{html}{...} blocks in the output
    /// By default, HTML-conditional content is excluded because it targets HTML
    /// renderers (CRAN HTML manual, pkgdown, etc.) and often contains raw HTML
    /// markup that produces noise in plain Markdown. Enable when targeting an
    /// HTML-capable renderer such as Quarto HTML output.
    #[arg(long)]
    include_html_output: bool,

    /// Prefer the ASCII representation of \eqn{}/\deqn{} equations over LaTeX
    /// math when one is present. Intended for renderers without math support,
    /// such as terminal pagers: \eqn becomes inline code and \deqn becomes a
    /// plain code block.
    #[arg(long)]
    prefer_ascii_math: bool,

    /// Output format for the Arguments section: list-table (Quarto list-table, default), grid-table
    /// (Pandoc grid table), pipe-table (GFM pipe table, inline only), or list (Markdown loose list).
    #[arg(long, value_enum)]
    arguments_format: Option<CliArgumentsFormat>,

    /// Generate topic index JSON file (directory mode only)
    /// Contains topic names, files, titles, aliases, and lifecycle stages
    #[arg(long, value_name = "FILE")]
    topic_index: Option<PathBuf>,

    /// Path to configuration file (default: _rd2qmd.toml in current directory)
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Ignore configuration file
    #[arg(long)]
    no_config: bool,
}

/// Subcommands
#[derive(Subcommand, Debug)]
enum Commands {
    /// Convert Rd files to Quarto Markdown (or standard Markdown / R Markdown)
    Convert(ConvertArgs),

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
struct IndexArgs {
    /// Input directory containing Rd files
    input: PathBuf,

    /// Output format extension (used for file field in JSON)
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Qmd)]
    format: OutputFormat,

    /// Process directories recursively
    #[arg(short, long)]
    recursive: bool,

    /// Include topics with \keyword{internal} in the index
    /// By default, internal topics are excluded (matching pkgdown behavior).
    #[arg(long)]
    include_internal: bool,
}

/// Arguments for the init subcommand
#[derive(Args, Debug)]
struct InitArgs {
    /// Output path for configuration file (default: _rd2qmd.toml)
    #[arg(short, long, default_value = "_rd2qmd.toml")]
    output: PathBuf,

    /// Overwrite existing file
    #[arg(long)]
    force: bool,

    /// Output JSON schema to stdout instead of creating config file
    #[arg(long)]
    schema: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.subcommand {
        Commands::Convert(args) => run_convert_command(&args, cli.verbose, cli.quiet),
        Commands::Index(args) => run_index_command(&args),
        Commands::Init(args) => run_init_command(&args, cli.quiet),
    }
}

/// Run the convert subcommand: convert Rd files to Markdown
fn run_convert_command(args: &ConvertArgs, verbose: bool, quiet: bool) -> Result<()> {
    // Load configuration file
    let config = load_config(args)?;

    // Merge: CLI > Config > Default
    // Format: CLI has default, so check if it was explicitly set or use config
    let format = merge_format(args, &config);
    let use_frontmatter = merge_frontmatter(args, &config);
    let use_pagetitle = merge_pagetitle(args, &config);
    let unqualified_link_url = merge_unqualified_link_url(args, &config);
    let external_link_url = merge_external_link_url(args, &config);
    // Config-only link options (no CLI flags)
    let internal_link_url = config.links.internal_link_url.clone();
    let package_urls = config.links.package_urls.clone();

    let input = args.input.clone();

    // Determine output extension and quarto_code_blocks based on format
    let output_extension = match format {
        OutputFormat::Qmd => "qmd",
        OutputFormat::Md => "md",
        OutputFormat::Rmd => "Rmd",
    };

    // quarto_code_blocks: CLI > Config > auto (based on format)
    let quarto_code_blocks = args
        .quarto_code_blocks
        .or(config.code.quarto_code_blocks)
        .unwrap_or(matches!(format, OutputFormat::Qmd | OutputFormat::Rmd));

    // exec_dontrun: CLI > Config > false
    let exec_dontrun = if args.exec_dontrun {
        true
    } else {
        config.code.exec_dontrun.unwrap_or(false)
    };

    // exec_donttest: CLI > Config > true (default is to execute donttest)
    let exec_donttest = if args.no_exec_donttest {
        false
    } else {
        config.code.exec_donttest.unwrap_or(true)
    };

    // Convert arguments table format: CLI > Config > Grid
    let arguments_format = merge_arguments_format(args, &config);

    // include_internal: CLI > Config > false (skip internal by default)
    let include_internal = if args.include_internal {
        true
    } else {
        config.output.include_internal.unwrap_or(false)
    };

    let include_html_output = args.include_html_output;

    // prefer_ascii_math: CLI > Config > false (LaTeX math by default)
    let prefer_ascii_math = if args.prefer_ascii_math {
        true
    } else {
        config.output.prefer_ascii_math.unwrap_or(false)
    };

    if input.is_file() {
        // Single file conversion (no alias resolution)
        convert_single_file(
            &input,
            args.output.as_deref(),
            output_extension,
            use_frontmatter,
            use_pagetitle,
            quarto_code_blocks,
            unqualified_link_url.as_deref(),
            external_link_url.as_deref(),
            package_urls,
            exec_dontrun,
            exec_donttest,
            include_html_output,
            prefer_ascii_math,
            arguments_format,
            verbose,
            quiet,
        )?;
    } else if input.is_dir() {
        // Build external package URL options
        let external_link_options = merge_external_link_options(args, &config);

        // Directory conversion (with alias resolution via rd2qmd-package)
        convert_directory(
            &input,
            args.output.as_deref(),
            output_extension,
            args.recursive,
            use_frontmatter,
            use_pagetitle,
            quarto_code_blocks,
            internal_link_url,
            unqualified_link_url,
            external_link_url,
            package_urls,
            external_link_options,
            exec_dontrun,
            exec_donttest,
            include_internal,
            include_html_output,
            prefer_ascii_math,
            arguments_format,
            args.topic_index.as_deref(),
            verbose,
            quiet,
            args.jobs,
        )?;
    } else {
        anyhow::bail!("Input path does not exist: {}", input.display());
    }

    Ok(())
}

/// Convert a single Rd file (without alias resolution)
#[allow(clippy::too_many_arguments)]
fn convert_single_file(
    input: &Path,
    output: Option<&Path>,
    output_extension: &str,
    use_frontmatter: bool,
    use_pagetitle: bool,
    quarto_code_blocks: bool,
    unqualified_link_url: Option<&str>,
    external_link_url: Option<&str>,
    package_urls: Option<std::collections::HashMap<String, String>>,
    exec_dontrun: bool,
    exec_donttest: bool,
    include_html_output: bool,
    prefer_ascii_math: bool,
    arguments_format: ArgumentsFormat,
    verbose: bool,
    quiet: bool,
) -> Result<()> {
    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => input.with_extension(output_extension),
    };

    if verbose {
        eprintln!(
            "Converting: {} -> {}",
            input.display(),
            output_path.display()
        );
    }

    let content = fs::read_to_string(input)
        .with_context(|| format!("Failed to read: {}", input.display()))?;

    // Build converter using RdConverter builder pattern
    let mut converter = RdConverter::new(&content)
        .frontmatter(use_frontmatter)
        .pagetitle(use_pagetitle)
        .quarto_code_blocks(quarto_code_blocks)
        .exec_dontrun(exec_dontrun)
        .exec_donttest(exec_donttest)
        .include_html_output(include_html_output)
        .prefer_ascii_math(prefer_ascii_math)
        .arguments_format(arguments_format);

    if let Some(template) = unqualified_link_url {
        converter = converter.unqualified_link_url(template);
    }

    if let Some(template) = external_link_url {
        converter = converter.external_link_url(template);
    }

    if let Some(urls) = package_urls {
        converter = converter.package_urls(urls);
    }

    let qmd = converter
        .convert()
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    fs::write(&output_path, &qmd)
        .with_context(|| format!("Failed to write: {}", output_path.display()))?;

    if !quiet {
        println!("{}", output_path.display());
    }

    Ok(())
}

/// Convert a directory of Rd files (with alias resolution)
#[allow(clippy::too_many_arguments)]
fn convert_directory(
    input: &Path,
    output: Option<&Path>,
    output_extension: &str,
    recursive: bool,
    use_frontmatter: bool,
    use_pagetitle: bool,
    quarto_code_blocks: bool,
    internal_link_url: Option<String>,
    unqualified_link_url: Option<String>,
    external_link_url: Option<String>,
    package_urls: Option<std::collections::HashMap<String, String>>,
    external_link_options: Option<ExternalLinkOptions>,
    exec_dontrun: bool,
    exec_donttest: bool,
    include_internal: bool,
    include_html_output: bool,
    prefer_ascii_math: bool,
    arguments_format: ArgumentsFormat,
    topic_index_path: Option<&Path>,
    verbose: bool,
    quiet: bool,
    jobs: Option<usize>,
) -> Result<()> {
    let output_dir = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| input.to_path_buf());

    // Load package with alias index
    if verbose {
        eprintln!("Scanning {} for Rd files...", input.display());
    }

    let package = RdPackage::from_directory(input, recursive)
        .with_context(|| format!("Failed to scan directory: {}", input.display()))?;

    if package.files().is_empty() {
        if !quiet {
            eprintln!("No .Rd files found in {}", input.display());
        }
        return Ok(());
    }

    if verbose {
        eprintln!("Found {} .Rd files", package.files().len());
        eprintln!(
            "Built alias index with {} entries",
            package.alias_index().len()
        );
    }

    // Configure conversion options
    let has_external_link_url = external_link_url.is_some();
    let options = PackageConvertOptions {
        output_dir,
        output_extension: output_extension.to_string(),
        frontmatter: use_frontmatter,
        pagetitle: use_pagetitle,
        quarto_code_blocks,
        parallel_jobs: jobs,
        internal_link_url,
        unqualified_link_url,
        // User-provided entries take precedence over auto-resolved URLs
        package_urls,
        external_link_url,
        exec_dontrun,
        exec_donttest,
        include_internal,
        include_html_output,
        prefer_ascii_math,
        arguments_format,
    };

    // Convert external link options
    // Build converter
    let mut converter = PackageConverter::new(&package, options);

    // Add external link resolution if configured
    if let Some(opts) = external_link_options {
        if opts.lib_paths.is_empty() {
            if verbose {
                eprintln!("No R library paths specified, skipping external link resolution");
            }
        } else {
            if verbose {
                eprintln!("External link resolution enabled");
            }
            converter = converter.with_external_links(PackageExternalLinkOptions {
                lib_paths: opts.lib_paths,
                cache_dir: opts.cache_dir,
            });
        }
    }

    // Execute conversion
    let FullConvertResult {
        conversion: result,
        fallbacks,
    } = converter
        .convert()
        .with_context(|| "Package conversion failed")?;

    // Display fallback warnings
    if !quiet && !fallbacks.is_empty() {
        display_fallback_warnings(&fallbacks, has_external_link_url, verbose);
    }

    // Print output files
    if !quiet {
        for path in &result.output_files {
            println!("{}", path.display());
        }
    }

    // Report errors
    for (file, error) in &result.failed_files {
        eprintln!("Error converting {}: {}", file.display(), error);
    }

    // Report skipped internal topics
    if verbose && !result.skipped_internal.is_empty() {
        for path in &result.skipped_internal {
            eprintln!("Skipped (internal): {}", path.display());
        }
    }

    if !quiet {
        let mut summary = format!(
            "Converted {} files, {} failed",
            result.success_count,
            result.failed_files.len()
        );
        if !result.skipped_internal.is_empty() {
            summary.push_str(&format!(
                ", {} skipped (internal)",
                result.skipped_internal.len()
            ));
        }
        eprintln!("{}", summary);
    }

    if !result.failed_files.is_empty() {
        anyhow::bail!("{} files failed to convert", result.failed_files.len());
    }

    // Generate topic index if requested
    if let Some(index_path) = topic_index_path {
        if verbose {
            eprintln!("Generating topic index...");
        }

        let index_options = TopicIndexOptions {
            output_extension: output_extension.to_string(),
            include_internal,
        };
        let index = generate_topic_index(&package, &index_options)
            .with_context(|| "Failed to generate topic index")?;

        let json = index
            .to_json()
            .with_context(|| "Failed to serialize topic index")?;

        fs::write(index_path, &json)
            .with_context(|| format!("Failed to write topic index: {}", index_path.display()))?;

        if !quiet {
            eprintln!("Topic index written to {}", index_path.display());
        }
    }

    Ok(())
}

/// Display fallback warnings for external package URL resolution
fn display_fallback_warnings(
    fallbacks: &std::collections::HashMap<String, FallbackReason>,
    has_external_link_url: bool,
    verbose: bool,
) {
    // What actually happens to links of unresolved packages depends on
    // whether the --external-link-url fallback is enabled
    let outcome = if has_external_link_url {
        "will use --external-link-url fallback"
    } else {
        "links will become plain inline code"
    };

    // Group fallbacks by reason
    let not_installed: Vec<_> = fallbacks
        .iter()
        .filter(|(_, r)| **r == FallbackReason::NotInstalled)
        .map(|(pkg, _)| pkg.as_str())
        .collect();
    let no_pkgdown: Vec<_> = fallbacks
        .iter()
        .filter(|(_, r)| **r == FallbackReason::NoPkgdownSite)
        .map(|(pkg, _)| pkg.as_str())
        .collect();

    if verbose {
        // Detailed warnings with package names
        for pkg in &not_installed {
            eprintln!("Warning: package '{}' is not installed, {}", pkg, outcome);
        }
        for pkg in &no_pkgdown {
            eprintln!(
                "Warning: package '{}' has no pkgdown site, {}",
                pkg, outcome
            );
        }
    } else {
        // Summary warnings
        if !not_installed.is_empty() {
            eprintln!(
                "Warning: {} package(s) not installed, {}: {}",
                not_installed.len(),
                outcome,
                not_installed.join(", ")
            );
        }
        if !no_pkgdown.is_empty() {
            eprintln!(
                "Warning: {} package(s) have no pkgdown site, {}: {}",
                no_pkgdown.len(),
                outcome,
                no_pkgdown.join(", ")
            );
        }
    }
}

/// Run the index subcommand: generate topic index JSON to stdout
fn run_index_command(args: &IndexArgs) -> Result<()> {
    if !args.input.is_dir() {
        anyhow::bail!("Input path is not a directory: {}", args.input.display());
    }

    let output_extension = match args.format {
        OutputFormat::Qmd => "qmd",
        OutputFormat::Md => "md",
        OutputFormat::Rmd => "Rmd",
    };

    let package = RdPackage::from_directory(&args.input, args.recursive)
        .with_context(|| format!("Failed to scan directory: {}", args.input.display()))?;

    if package.files().is_empty() {
        anyhow::bail!("No .Rd files found in {}", args.input.display());
    }

    let index_options = TopicIndexOptions {
        output_extension: output_extension.to_string(),
        include_internal: args.include_internal,
    };

    let index = generate_topic_index(&package, &index_options)
        .with_context(|| "Failed to generate topic index")?;

    let json = index
        .to_json()
        .with_context(|| "Failed to serialize topic index")?;

    // Output to stdout for piping to jq etc.
    println!("{}", json);

    Ok(())
}

/// Run the init subcommand: generate configuration file
fn run_init_command(args: &InitArgs, quiet: bool) -> Result<()> {
    // Handle --schema flag: output JSON schema to stdout
    if args.schema {
        let schema = Config::json_schema_string()?;
        println!("{}", schema);
        return Ok(());
    }

    if args.output.exists() && !args.force {
        anyhow::bail!(
            "Configuration file already exists: {}\nUse --force to overwrite.",
            args.output.display()
        );
    }

    let config = Config::sample();
    let config_content = config.to_toml_with_schema()?;

    fs::write(&args.output, &config_content).with_context(|| {
        format!(
            "Failed to write configuration file: {}",
            args.output.display()
        )
    })?;

    if !quiet {
        eprintln!("Created configuration file: {}", args.output.display());
    }
    Ok(())
}

// ============================================================================
// Configuration loading and merging helpers
// ============================================================================

/// Load configuration file based on CLI options
fn load_config(args: &ConvertArgs) -> Result<Config> {
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
fn merge_format(args: &ConvertArgs, config: &Config) -> OutputFormat {
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
fn merge_frontmatter(args: &ConvertArgs, config: &Config) -> bool {
    // CLI --no-frontmatter explicitly disables
    if args.no_frontmatter {
        return false;
    }
    // Config value if specified, otherwise CLI default (true)
    config.output.frontmatter.unwrap_or(args.frontmatter)
}

/// Merge pagetitle setting
fn merge_pagetitle(args: &ConvertArgs, config: &Config) -> bool {
    // CLI --no-pagetitle explicitly disables
    if args.no_pagetitle {
        return false;
    }
    // Config value if specified, otherwise default (true)
    config.output.pagetitle.unwrap_or(true)
}

/// Merge unqualified link URL: CLI > Config > Default
fn merge_unqualified_link_url(args: &ConvertArgs, config: &Config) -> Option<String> {
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
fn merge_external_link_url(args: &ConvertArgs, config: &Config) -> Option<String> {
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
fn merge_arguments_format(args: &ConvertArgs, config: &Config) -> ArgumentsFormat {
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
fn merge_external_link_options(args: &ConvertArgs, config: &Config) -> Option<ExternalLinkOptions> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
