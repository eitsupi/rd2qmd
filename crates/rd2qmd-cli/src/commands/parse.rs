//! The `parse` subcommand: parses Rd files to AST JSON.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use rd2qmd_core::{RdAstEnvelope, extract_rd_metadata};
use rd2qmd_package::export_package_ast;

use super::display_diagnostic;
use crate::cli::ParseArgs;

/// Run the parse subcommand: parse Rd files to AST JSON
pub(crate) fn run_parse_command(args: &ParseArgs, verbose: bool, quiet: bool) -> Result<()> {
    let input = args.input.clone();

    if input.is_file() {
        parse_single_file(&input, args.output.as_deref(), verbose, quiet)?;
    } else if input.is_dir() {
        let output_dir = args.output.clone().unwrap_or_else(|| input.to_path_buf());

        if verbose {
            eprintln!("Scanning {} for Rd files...", input.display());
        }

        let result = export_package_ast(&input, args.recursive, &output_dir, args.jobs)
            .with_context(|| format!("Failed to scan directory: {}", input.display()))?;

        if !quiet {
            for file_diagnostics in &result.diagnostics {
                for diagnostic in &file_diagnostics.diagnostics {
                    display_diagnostic(&file_diagnostics.file, diagnostic);
                }
            }
        }

        if !quiet {
            for path in &result.output_files {
                println!("{}", path.display());
            }
        }

        for (file, error) in &result.failed_files {
            eprintln!("Error parsing {}: {}", file.display(), error);
        }

        if !quiet {
            eprintln!(
                "Parsed {} files, {} failed",
                result.success_count,
                result.failed_files.len()
            );
        }

        if !result.failed_files.is_empty() {
            anyhow::bail!("{} files failed to parse", result.failed_files.len());
        }
    } else {
        anyhow::bail!("Input path does not exist: {}", input.display());
    }

    Ok(())
}

/// Parse a single Rd file to an AST JSON envelope
fn parse_single_file(
    input: &Path,
    output: Option<&Path>,
    verbose: bool,
    quiet: bool,
) -> Result<()> {
    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => input.with_extension("json"),
    };

    if verbose {
        eprintln!("Parsing: {} -> {}", input.display(), output_path.display());
    }

    let content = fs::read_to_string(input)
        .with_context(|| format!("Failed to read: {}", input.display()))?;

    let parsed = rd2qmd_source::parse(&content).map_err(|e| anyhow::anyhow!("Parse error: {e}"))?;
    if !quiet {
        for diagnostic in parsed.diagnostics() {
            display_diagnostic(input, diagnostic);
        }
    }
    let doc = parsed.document().clone();
    let source_files = extract_rd_metadata(&doc).source_files;

    let source = input
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());
    let envelope = RdAstEnvelope::new(doc, source, source_files);
    let json = envelope
        .to_json_pretty()
        .with_context(|| "Failed to serialize AST JSON")?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    fs::write(&output_path, &json)
        .with_context(|| format!("Failed to write: {}", output_path.display()))?;

    if !quiet {
        println!("{}", output_path.display());
    }

    Ok(())
}
