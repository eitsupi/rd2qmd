//! rd2qmd: CLI tool to convert Rd files to Quarto Markdown

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

use rd2qmd_core::writer::Frontmatter;
use rd2qmd_core::{ConverterOptions, WriterOptions, mdast_to_qmd, parse, rd_to_mdast_with_options};
use rd2qmd_package::{PackageConvertOptions, RdPackage, convert_package};

#[cfg(feature = "external-links")]
use rd2qmd_package::{PackageUrlResolver, PackageUrlResolverOptions, collect_external_packages};

/// Options for external package link resolution
#[derive(Debug, Clone)]
struct ExternalLinkOptions {
    lib_paths: Vec<PathBuf>,
    cache_dir: Option<PathBuf>,
    fallback_url: Option<String>,
}

#[derive(Parser, Debug)]
#[command(name = "rd2qmd")]
#[command(about = "Convert Rd files to Quarto Markdown")]
#[command(version)]
#[command(after_help = "Examples:
  rd2qmd file.Rd                    # Convert single file to file.qmd
  rd2qmd file.Rd -o output.qmd      # Convert to specific output file
  rd2qmd man/ -o docs/              # Convert directory (with alias resolution)
  rd2qmd man/ -o docs/ -j4          # Use 4 parallel jobs")]
struct Cli {
    /// Input Rd file or directory
    input: PathBuf,

    /// Output file or directory
    #[arg(short, long)]
    output: Option<PathBuf>,

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

    /// Use Quarto {r} code blocks instead of r
    #[arg(long, default_value = "true")]
    quarto_code_blocks: bool,

    /// URL pattern for unresolved links (fallback for base R documentation)
    /// Use {topic} as placeholder for the topic name.
    #[arg(long, value_name = "URL_PATTERN", default_value = "https://rdrr.io/r/base/{topic}.html")]
    unresolved_link_url: String,

    /// Disable fallback URL for unresolved links
    #[arg(long, conflicts_with = "unresolved_link_url")]
    no_unresolved_link_url: bool,

    /// R library path to search for external packages (can be specified multiple times)
    #[cfg(feature = "external-links")]
    #[arg(long = "r-lib-path", value_name = "PATH")]
    r_lib_paths: Vec<PathBuf>,

    /// Cache directory for pkgdown.yml files (default: system temp directory)
    #[cfg(feature = "external-links")]
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,

    /// Disable external package link resolution
    #[cfg(feature = "external-links")]
    #[arg(long)]
    no_external_links: bool,

    /// Fallback URL pattern for external packages without pkgdown sites
    /// Use {package} and {topic} as placeholders
    #[cfg(feature = "external-links")]
    #[arg(long, value_name = "URL_PATTERN", default_value = "https://rdrr.io/pkg/{package}/man/{topic}.html")]
    external_package_fallback: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Quiet mode - only show errors
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let use_frontmatter = cli.frontmatter && !cli.no_frontmatter;
    let unresolved_link_url = if cli.no_unresolved_link_url {
        None
    } else {
        Some(cli.unresolved_link_url.clone())
    };

    if cli.input.is_file() {
        // Single file conversion (no alias resolution)
        convert_single_file(
            &cli.input,
            cli.output.as_deref(),
            use_frontmatter,
            cli.quarto_code_blocks,
            unresolved_link_url.as_deref(),
            cli.verbose,
            cli.quiet,
        )?;
    } else if cli.input.is_dir() {
        // Build external package URL options
        #[cfg(feature = "external-links")]
        let external_link_options = if cli.no_external_links {
            None
        } else {
            Some(ExternalLinkOptions {
                lib_paths: cli.r_lib_paths.clone(),
                cache_dir: cli.cache_dir.clone(),
                fallback_url: Some(cli.external_package_fallback.clone()),
            })
        };
        #[cfg(not(feature = "external-links"))]
        let external_link_options: Option<ExternalLinkOptions> = None;

        // Directory conversion (with alias resolution via rd2qmd-package)
        convert_directory(
            &cli.input,
            cli.output.as_deref(),
            cli.recursive,
            use_frontmatter,
            cli.quarto_code_blocks,
            unresolved_link_url,
            external_link_options,
            cli.verbose,
            cli.quiet,
            cli.jobs,
        )?;
    } else {
        anyhow::bail!("Input path does not exist: {}", cli.input.display());
    }

    Ok(())
}

/// Convert a single Rd file to QMD (without alias resolution)
fn convert_single_file(
    input: &Path,
    output: Option<&Path>,
    use_frontmatter: bool,
    quarto_code_blocks: bool,
    unresolved_link_url: Option<&str>,
    verbose: bool,
    quiet: bool,
) -> Result<()> {
    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => input.with_extension("qmd"),
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

    let qmd = convert_rd_to_qmd(&content, use_frontmatter, quarto_code_blocks, None, unresolved_link_url)?;

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
    recursive: bool,
    use_frontmatter: bool,
    quarto_code_blocks: bool,
    unresolved_link_url: Option<String>,
    external_link_options: Option<ExternalLinkOptions>,
    verbose: bool,
    quiet: bool,
    jobs: Option<usize>,
) -> Result<()> {
    let output_dir = output.map(|p| p.to_path_buf()).unwrap_or_else(|| input.to_path_buf());

    // Load package with alias index
    if verbose {
        eprintln!("Scanning {} for Rd files...", input.display());
    }

    let package = RdPackage::from_directory(input, recursive)
        .with_context(|| format!("Failed to scan directory: {}", input.display()))?;

    if package.files.is_empty() {
        if !quiet {
            eprintln!("No .Rd files found in {}", input.display());
        }
        return Ok(());
    }

    if verbose {
        eprintln!("Found {} .Rd files", package.files.len());
        eprintln!("Built alias index with {} entries", package.alias_index.len());
    }

    // Resolve external package URLs if enabled
    #[cfg(feature = "external-links")]
    let external_package_urls = if let Some(opts) = external_link_options {
        if opts.lib_paths.is_empty() {
            if verbose {
                eprintln!("No R library paths specified, skipping external link resolution");
            }
            None
        } else {
            if verbose {
                eprintln!("Collecting external package references...");
            }
            let external_packages = collect_external_packages(&package);
            if verbose {
                eprintln!("Found {} external packages: {:?}", external_packages.len(), external_packages);
            }

            if external_packages.is_empty() {
                None
            } else {
                if verbose {
                    eprintln!("Resolving external package URLs...");
                }
                let mut resolver = PackageUrlResolver::new(PackageUrlResolverOptions {
                    lib_paths: opts.lib_paths,
                    cache_dir: opts.cache_dir,
                    fallback_url: opts.fallback_url,
                    enable_http: true,
                });
                let urls = resolver.resolve_packages(&external_packages);
                if verbose {
                    eprintln!("Resolved {} package URLs", urls.len());
                }
                if urls.is_empty() {
                    None
                } else {
                    Some(urls)
                }
            }
        }
    } else {
        None
    };
    #[cfg(not(feature = "external-links"))]
    let external_package_urls: Option<HashMap<String, String>> = {
        let _ = external_link_options; // Suppress unused warning
        None
    };

    // Configure conversion options
    let options = PackageConvertOptions {
        output_dir,
        output_extension: "qmd".to_string(),
        frontmatter: use_frontmatter,
        quarto_code_blocks,
        parallel_jobs: jobs,
        unresolved_link_url,
        external_package_urls,
    };

    // Convert package
    let result = convert_package(&package, &options)
        .with_context(|| "Package conversion failed")?;

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

    if !quiet {
        eprintln!(
            "Converted {} files, {} failed",
            result.success_count,
            result.failed_files.len()
        );
    }

    if !result.failed_files.is_empty() {
        anyhow::bail!("{} files failed to convert", result.failed_files.len());
    }

    Ok(())
}

/// Core conversion function for single file
fn convert_rd_to_qmd(
    rd_content: &str,
    use_frontmatter: bool,
    quarto_code_blocks: bool,
    alias_map: Option<std::collections::HashMap<String, String>>,
    unresolved_link_url: Option<&str>,
) -> Result<String> {
    let doc = parse(rd_content).map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    let converter_options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
        alias_map,
        unresolved_link_url: unresolved_link_url.map(|s| s.to_string()),
        external_package_urls: None, // Single file mode doesn't resolve external packages
    };
    let mdast = rd_to_mdast_with_options(&doc, &converter_options);

    // Extract title for frontmatter
    let title = doc
        .get_section(&rd2qmd_core::SectionTag::Title)
        .map(|s| extract_text(&s.content));

    let options = WriterOptions {
        frontmatter: if use_frontmatter {
            Some(Frontmatter {
                title,
                format: None,
            })
        } else {
            None
        },
        quarto_code_blocks,
    };

    Ok(mdast_to_qmd(&mdast, &options))
}

/// Extract plain text from Rd nodes
fn extract_text(nodes: &[rd2qmd_core::RdNode]) -> String {
    use rd2qmd_core::RdNode;

    let mut result = String::new();
    for node in nodes {
        match node {
            RdNode::Text(s) => result.push_str(s),
            RdNode::Code(children) | RdNode::Emph(children) | RdNode::Strong(children) => {
                result.push_str(&extract_text(children));
            }
            _ => {}
        }
    }
    result.trim().to_string()
}
