//! rd2qmd: CLI tool to convert Rd files to Quarto Markdown

mod cli;
mod commands;
mod config;
mod config_merge;
#[cfg(test)]
mod tests;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};
use commands::{run_convert_command, run_index_command, run_init_command, run_parse_command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.subcommand {
        Commands::Convert(args) => run_convert_command(&args, cli.verbose, cli.quiet),
        Commands::Parse(args) => run_parse_command(&args, cli.verbose, cli.quiet),
        Commands::Index(args) => run_index_command(&args, cli.quiet),
        Commands::Init(args) => run_init_command(&args, cli.quiet),
    }
}
