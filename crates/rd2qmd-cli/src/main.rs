//! rd2qmd: CLI tool to convert Rd files to Quarto Markdown

use anyhow::{Context, Result};
use clap::Parser;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use rd2qmd_core::writer::Frontmatter;
use rd2qmd_core::{ConverterOptions, WriterOptions, mdast_to_qmd, parse, rd_to_mdast_with_options};

#[derive(Parser, Debug)]
#[command(name = "rd2qmd")]
#[command(about = "Convert Rd files to Quarto Markdown")]
#[command(version)]
#[command(after_help = "Examples:
  rd2qmd file.Rd                    # Convert single file to file.qmd
  rd2qmd file.Rd -o output.qmd      # Convert to specific output file
  rd2qmd man/ -o docs/              # Convert directory
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

    if cli.input.is_file() {
        convert_file(
            &cli.input,
            cli.output.as_deref(),
            use_frontmatter,
            cli.quarto_code_blocks,
            cli.verbose,
            cli.quiet,
        )?;
    } else if cli.input.is_dir() {
        convert_directory(
            &cli.input,
            cli.output.as_deref(),
            cli.recursive,
            use_frontmatter,
            cli.quarto_code_blocks,
            cli.verbose,
            cli.quiet,
            cli.jobs,
        )?;
    } else {
        anyhow::bail!("Input path does not exist: {}", cli.input.display());
    }

    Ok(())
}

/// Convert a single Rd file to QMD
fn convert_file(
    input: &Path,
    output: Option<&Path>,
    use_frontmatter: bool,
    quarto_code_blocks: bool,
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

    let qmd = convert_rd_to_qmd(&content, use_frontmatter, quarto_code_blocks)?;

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

/// Convert a directory of Rd files
fn convert_directory(
    input: &Path,
    output: Option<&Path>,
    recursive: bool,
    use_frontmatter: bool,
    quarto_code_blocks: bool,
    verbose: bool,
    quiet: bool,
    jobs: Option<usize>,
) -> Result<()> {
    let output_dir = output.unwrap_or(input);

    let files = collect_rd_files(input, recursive)?;

    if files.is_empty() {
        if !quiet {
            eprintln!("No .Rd files found in {}", input.display());
        }
        return Ok(());
    }

    let total = files.len();
    if verbose {
        eprintln!("Found {} .Rd files", total);
    }

    // Configure thread pool if jobs specified
    if let Some(n) = jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .ok(); // Ignore error if already initialized
    }

    // Atomic counters for thread-safe progress tracking
    let success = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    // Parallel conversion
    let errors: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let relative = file.strip_prefix(input).unwrap_or(file);
            let output_file = output_dir.join(relative).with_extension("qmd");

            match convert_file_inner(file, &output_file, use_frontmatter, quarto_code_blocks) {
                Ok(()) => {
                    success.fetch_add(1, Ordering::Relaxed);
                    if !quiet {
                        println!("{}", output_file.display());
                    }
                    None
                }
                Err(e) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    Some((file.clone(), e))
                }
            }
        })
        .collect();

    // Report errors
    for (file, e) in &errors {
        eprintln!("Error converting {}: {}", file.display(), e);
    }

    let success_count = success.load(Ordering::Relaxed);
    let failed_count = failed.load(Ordering::Relaxed);

    if !quiet {
        eprintln!("Converted {} files, {} failed", success_count, failed_count);
    }

    if failed_count > 0 {
        anyhow::bail!("{} files failed to convert", failed_count);
    }

    Ok(())
}

/// Inner conversion function that doesn't print (for parallel use)
fn convert_file_inner(
    input: &Path,
    output: &Path,
    use_frontmatter: bool,
    quarto_code_blocks: bool,
) -> Result<()> {
    let content = fs::read_to_string(input)
        .with_context(|| format!("Failed to read: {}", input.display()))?;

    let qmd = convert_rd_to_qmd(&content, use_frontmatter, quarto_code_blocks)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    fs::write(output, &qmd).with_context(|| format!("Failed to write: {}", output.display()))?;

    Ok(())
}

/// Collect all .Rd files in a directory
fn collect_rd_files(dir: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in
        fs::read_dir(dir).with_context(|| format!("Failed to read directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("rd") {
                    files.push(path);
                }
            }
        } else if path.is_dir() && recursive {
            files.extend(collect_rd_files(&path, recursive)?);
        }
    }

    Ok(files)
}

/// Core conversion function
fn convert_rd_to_qmd(
    rd_content: &str,
    use_frontmatter: bool,
    quarto_code_blocks: bool,
) -> Result<String> {
    let doc = parse(rd_content).map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    let converter_options = ConverterOptions {
        link_extension: Some("qmd".to_string()),
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
