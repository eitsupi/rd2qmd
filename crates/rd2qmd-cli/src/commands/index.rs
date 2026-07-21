//! The `index` subcommand: generates topic index JSON to stdout.

use anyhow::{Context, Result};

use rd2qmd_package::{RdPackage, TopicIndexOptions, generate_topic_index_with_diagnostics};

use super::display_diagnostic;
use crate::cli::{IndexArgs, OutputFormat};

/// Run the index subcommand: generate topic index JSON to stdout
pub(crate) fn run_index_command(args: &IndexArgs, quiet: bool) -> Result<()> {
    if !args.input.is_dir() {
        anyhow::bail!("Input path is not a directory: {}", args.input.display());
    }

    let output_extension = match args.format {
        OutputFormat::Qmd => "qmd",
        OutputFormat::Md => "md",
        OutputFormat::Rmd => "Rmd",
    };

    let package = RdPackage::from_directory_with_format(
        &args.input,
        args.recursive,
        args.input_format.into(),
    )
    .with_context(|| format!("Failed to scan directory: {}", args.input.display()))?;

    if package.files().is_empty() {
        anyhow::bail!(
            "No {} files found in {}",
            args.input_format.file_extension(),
            args.input.display()
        );
    }

    let index_options = TopicIndexOptions {
        output_extension: output_extension.to_string(),
        include_internal: args.include_internal,
    };

    let result = generate_topic_index_with_diagnostics(&package, &index_options)
        .with_context(|| "Failed to generate topic index")?;

    if !quiet {
        for file_diagnostics in &result.diagnostics {
            for diagnostic in &file_diagnostics.diagnostics {
                display_diagnostic(&file_diagnostics.file, diagnostic);
            }
        }
    }

    let json = result
        .index
        .to_json()
        .with_context(|| "Failed to serialize topic index")?;

    // Output to stdout for piping to jq etc.
    println!("{}", json);

    Ok(())
}
