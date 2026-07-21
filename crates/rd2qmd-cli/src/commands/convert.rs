//! The `convert` subcommand: converts Rd files to Quarto/standard/R Markdown.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use rd2qmd_core::{
    ArgumentsFormat, CodeExecutionOptions, FrontmatterOptions, LinkOptions, RdAstEnvelope,
    RdConvertOptions, convert_rd_document,
};
use rd2qmd_package::{
    ExternalLinkOptions as PackageExternalLinkOptions, FallbackReason, FullConvertResult,
    PackageConvertOptions, PackageConverter, RdPackage, TopicIndexOptions, generate_topic_index,
};

use super::display_diagnostic;
use crate::cli::{ConvertArgs, ExternalLinkOptions, InputFormat, OutputFormat};
use crate::config_merge::{
    load_config, merge_arguments_format, merge_external_link_options, merge_external_link_url,
    merge_format, merge_frontmatter, merge_pagetitle, merge_unqualified_link_url,
};

/// Run the convert subcommand: convert Rd files to Markdown
pub(crate) fn run_convert_command(args: &ConvertArgs, verbose: bool, quiet: bool) -> Result<()> {
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
            args.input_format,
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
            args.input_format,
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
    input_format: InputFormat,
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

    let is_ast = input_format == InputFormat::Ast
        || input
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));

    let qmd = if is_ast {
        let content = fs::read_to_string(input)
            .with_context(|| format!("Failed to read: {}", input.display()))?;

        let envelope = RdAstEnvelope::from_json(&content)
            .with_context(|| format!("Failed to read AST JSON envelope: {}", input.display()))?;

        let options = RdConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: use_frontmatter,
                pagetitle: use_pagetitle,
            },
            code: CodeExecutionOptions {
                quarto_code_blocks,
                exec_dontrun,
                exec_donttest,
            },
            links: LinkOptions {
                internal_link_url: None,
                unqualified_link_url: unqualified_link_url.map(|s| s.to_string()),
                external_link_url: external_link_url.map(|s| s.to_string()),
                alias_map: None,
                package_urls,
            },
            arguments_format,
            include_html_output,
            prefer_ascii_math,
            // The envelope's own `source_files` (set explicitly by whatever
            // produced this AST JSON) is authoritative and may not match what
            // AST-embedded generation-header extraction would derive from
            // `envelope.document` alone.
            source_files_override: Some(envelope.source_files.clone()),
        };
        convert_rd_document(&envelope.document, &options)
    } else {
        let content = fs::read_to_string(input)
            .with_context(|| format!("Failed to read: {}", input.display()))?;

        let parsed =
            rd2qmd_source::parse(&content).map_err(|e| anyhow::anyhow!("Parse error: {e}"))?;
        if !quiet {
            for diagnostic in parsed.diagnostics() {
                display_diagnostic(input, diagnostic);
            }
        }
        let doc = parsed.document();
        let options = RdConvertOptions {
            frontmatter: FrontmatterOptions {
                enabled: use_frontmatter,
                pagetitle: use_pagetitle,
            },
            code: CodeExecutionOptions {
                quarto_code_blocks,
                exec_dontrun,
                exec_donttest,
            },
            links: LinkOptions {
                internal_link_url: None,
                unqualified_link_url: unqualified_link_url.map(str::to_owned),
                external_link_url: external_link_url.map(str::to_owned),
                alias_map: None,
                package_urls,
            },
            arguments_format,
            include_html_output,
            prefer_ascii_math,
            source_files_override: None,
        };
        convert_rd_document(doc, &options)
    };

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
    input_format: InputFormat,
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
        eprintln!(
            "Scanning {} for {} files...",
            input.display(),
            input_format.file_extension()
        );
    }

    let package = RdPackage::from_directory_with_format(input, recursive, input_format.into())
        .with_context(|| format!("Failed to scan directory: {}", input.display()))?;

    if package.files().is_empty() {
        if !quiet {
            eprintln!(
                "No {} files found in {}",
                input_format.file_extension(),
                input.display()
            );
        }
        return Ok(());
    }

    if verbose {
        eprintln!(
            "Found {} {} files",
            package.files().len(),
            input_format.file_extension()
        );
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

    if !quiet {
        for file_diagnostics in &result.diagnostics {
            for diagnostic in &file_diagnostics.diagnostics {
                display_diagnostic(&file_diagnostics.file, diagnostic);
            }
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
