//! rd2qmd: CLI tool to convert Rd files to Quarto Markdown

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "rd2qmd")]
#[command(about = "Convert Rd files to Quarto Markdown")]
#[command(version)]
struct Cli {
    /// Input Rd file or directory
    #[arg(required = true)]
    input: String,

    /// Output directory (defaults to current directory)
    #[arg(short, long)]
    output: Option<String>,

    /// Number of parallel jobs (defaults to number of CPUs)
    #[arg(short, long)]
    jobs: Option<usize>,
}

fn main() {
    let cli = Cli::parse();
    println!("rd2qmd v{}", env!("CARGO_PKG_VERSION"));
    println!("Input: {}", cli.input);
    if let Some(output) = &cli.output {
        println!("Output: {}", output);
    }
}
