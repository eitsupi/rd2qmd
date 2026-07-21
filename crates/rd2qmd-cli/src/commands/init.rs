//! The `init` subcommand: generates a configuration file.

use anyhow::{Context, Result};
use std::fs;

use crate::cli::InitArgs;
use crate::config::Config;

/// Run the init subcommand: generate configuration file
pub(crate) fn run_init_command(args: &InitArgs, quiet: bool) -> Result<()> {
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
