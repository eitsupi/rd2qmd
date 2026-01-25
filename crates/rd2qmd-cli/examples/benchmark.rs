//! Benchmark for rd2qmd conversion performance
//!
//! Usage:
//!   cargo run --release --example benchmark -- <man-dir> [options]
//!
//! Example:
//!   # Clone ggplot2 and run benchmark
//!   git clone --depth 1 https://github.com/tidyverse/ggplot2 /tmp/ggplot2
//!   cargo run --release --example benchmark -- /tmp/ggplot2/man
//!
//!   # With external link resolution
//!   cargo run --release --example benchmark -- /tmp/ggplot2/man --r-lib-path /usr/local/lib/R/site-library

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use rd2qmd_package::{PackageConvertOptions, RdPackage, convert_package};

#[cfg(feature = "external-links")]
use rd2qmd_package::{PackageUrlResolver, PackageUrlResolverOptions, collect_external_packages};

#[derive(Parser, Debug)]
#[command(name = "benchmark")]
#[command(about = "Benchmark rd2qmd conversion performance")]
struct Args {
    /// Input directory containing .Rd files
    man_dir: PathBuf,

    /// R library path for external link resolution (can be specified multiple times)
    #[cfg(feature = "external-links")]
    #[arg(long = "r-lib-path", value_name = "PATH")]
    r_lib_paths: Vec<PathBuf>,

    /// Cache directory for pkgdown.yml files
    #[cfg(feature = "external-links")]
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,

    /// Number of benchmark iterations
    #[arg(long, default_value = "3")]
    iterations: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Load package
    println!("Loading .Rd files from {}...", args.man_dir.display());
    let package =
        RdPackage::from_directory(&args.man_dir, false).context("Failed to load Rd files")?;

    println!("Found {} .Rd files", package.files.len());
    println!(
        "Built alias index with {} entries",
        package.alias_index.len()
    );
    println!();

    // Create temp output directory
    let output_dir = std::env::temp_dir().join("rd2qmd_benchmark");
    let cache_dir = std::env::temp_dir().join("rd2qmd_benchmark_cache");

    // Benchmark without external links
    println!("=== Without external link resolution ===");
    println!();
    println!("{:<45} {:>8}", "Configuration", "Time");
    println!("{:<45} {:>8}", "-------------", "----");

    for jobs in [1, 2, 4] {
        let times = run_benchmark(&package, &output_dir, jobs, None, args.iterations)?;
        let avg = average_duration(&times);
        println!(
            "{:<45} {:>7.2}s",
            format!("Jobs: {}", jobs),
            avg.as_secs_f64()
        );
    }
    println!();

    // Benchmark with external links if r-lib-path is provided
    #[cfg(feature = "external-links")]
    if !args.r_lib_paths.is_empty() {
        // Collect external packages
        println!("Collecting external package references...");
        let external_packages = collect_external_packages(&package);
        println!("Found {} external packages", external_packages.len());
        println!();

        // Cold cache benchmark (includes URL resolution time)
        println!("=== With external link resolution (cold cache) ===");
        println!();
        let _ = std::fs::remove_dir_all(&cache_dir);
        std::fs::create_dir_all(&cache_dir)?;

        let start = Instant::now();
        let external_urls =
            resolve_external_urls(&external_packages, &args.r_lib_paths, Some(&cache_dir))?;
        run_single_benchmark(&package, &output_dir, 1, Some(&external_urls))?;
        let cold_cache_time = start.elapsed();
        println!(
            "{:<45} {:>7.2}s (includes HTTP fetches)",
            "Jobs: 1 (cold cache)",
            cold_cache_time.as_secs_f64()
        );
        println!();

        // Warm cache benchmark (re-resolve URLs from cache + convert)
        println!("=== With external link resolution (warm cache) ===");
        println!();
        println!("{:<45} {:>8}", "Configuration", "Time");
        println!("{:<45} {:>8}", "-------------", "----");

        for jobs in [1, 2, 4] {
            let times = run_benchmark_with_url_resolution(
                &package,
                &output_dir,
                jobs,
                &external_packages,
                &args.r_lib_paths,
                Some(&cache_dir),
                args.iterations,
            )?;
            let avg = average_duration(&times);
            println!(
                "{:<45} {:>7.2}s",
                format!("Jobs: {}", jobs),
                avg.as_secs_f64()
            );
        }
        println!();
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&output_dir);
    let _ = std::fs::remove_dir_all(&cache_dir);

    println!("Done.");
    Ok(())
}

fn run_benchmark(
    package: &RdPackage,
    output_dir: &std::path::Path,
    jobs: usize,
    external_urls: Option<&std::collections::HashMap<String, String>>,
    iterations: usize,
) -> Result<Vec<Duration>> {
    let mut times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();
        run_single_benchmark(package, output_dir, jobs, external_urls)?;
        times.push(start.elapsed());
    }

    Ok(times)
}

#[cfg(feature = "external-links")]
fn run_benchmark_with_url_resolution(
    package: &RdPackage,
    output_dir: &std::path::Path,
    jobs: usize,
    external_packages: &std::collections::HashSet<String>,
    lib_paths: &[PathBuf],
    cache_dir: Option<&std::path::Path>,
    iterations: usize,
) -> Result<Vec<Duration>> {
    let mut times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();
        let external_urls = resolve_external_urls(external_packages, lib_paths, cache_dir)?;
        run_single_benchmark(package, output_dir, jobs, Some(&external_urls))?;
        times.push(start.elapsed());
    }

    Ok(times)
}

fn run_single_benchmark(
    package: &RdPackage,
    output_dir: &std::path::Path,
    jobs: usize,
    external_urls: Option<&std::collections::HashMap<String, String>>,
) -> Result<()> {
    let _ = std::fs::remove_dir_all(output_dir);
    std::fs::create_dir_all(output_dir)?;

    let options = PackageConvertOptions {
        output_dir: output_dir.to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: true,
        pagetitle: true,
        quarto_code_blocks: true,
        parallel_jobs: Some(jobs),
        unresolved_link_url: Some("https://rdrr.io/r/base/{topic}.html".to_string()),
        external_package_urls: external_urls.cloned(),
        exec_dontrun: false,
        exec_donttest: true, // pkgdown-compatible default
    };

    convert_package(package, &options)?;
    Ok(())
}

#[cfg(feature = "external-links")]
fn resolve_external_urls(
    packages: &std::collections::HashSet<String>,
    lib_paths: &[PathBuf],
    cache_dir: Option<&std::path::Path>,
) -> Result<std::collections::HashMap<String, String>> {
    let mut resolver = PackageUrlResolver::new(PackageUrlResolverOptions {
        lib_paths: lib_paths.to_vec(),
        cache_dir: cache_dir.map(|p| p.to_path_buf()),
        fallback_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
        enable_http: true,
    });

    let result = resolver.resolve_packages(packages);
    Ok(result.urls)
}

fn average_duration(durations: &[Duration]) -> Duration {
    if durations.is_empty() {
        return Duration::ZERO;
    }
    let total: Duration = durations.iter().sum();
    total / durations.len() as u32
}
