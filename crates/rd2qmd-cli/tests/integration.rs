//! Integration tests for rd2qmd conversion

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn rd2qmd_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/rd2qmd")
}

/// Run rd2qmd on a fixture file and return the output
fn convert_fixture(name: &str, args: &[&str]) -> String {
    let input = fixtures_dir().join(format!("{}.Rd", name));
    // Use a unique temp file for each invocation to avoid race conditions
    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let ext = if args.contains(&"md") { "md" } else { "qmd" };
    let output = std::env::temp_dir().join(format!("rd2qmd_test_{}_{}_{}_{}.{}", name, pid, unique_id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), ext));

    let mut cmd = Command::new(rd2qmd_binary());
    cmd.arg(&input).arg("-o").arg(&output);
    for arg in args {
        cmd.arg(arg);
    }

    let status = cmd.status().expect("Failed to run rd2qmd");
    assert!(status.success(), "rd2qmd failed with status: {}", status);

    let content = fs::read_to_string(&output).expect("Failed to read output file");
    // Clean up
    let _ = fs::remove_file(&output);
    content
}

#[test]
fn test_simple_conversion() {
    let output = convert_fixture("simple", &[]);
    insta::assert_snapshot!("simple_qmd", output);
}

#[test]
fn test_simple_to_md() {
    let output = convert_fixture("simple", &["-f", "md"]);
    insta::assert_snapshot!("simple_md", output);
}

#[test]
fn test_simple_no_frontmatter() {
    let output = convert_fixture("simple", &["--no-frontmatter"]);
    insta::assert_snapshot!("simple_no_frontmatter", output);
}

#[test]
fn test_simple_no_pagetitle() {
    let output = convert_fixture("simple", &["--no-pagetitle"]);
    insta::assert_snapshot!("simple_no_pagetitle", output);
}

#[test]
fn test_with_links() {
    let output = convert_fixture("with_links", &[]);
    insta::assert_snapshot!("with_links_qmd", output);
}

#[test]
fn test_formatting() {
    let output = convert_fixture("formatting", &[]);
    insta::assert_snapshot!("formatting_qmd", output);
}

#[test]
fn test_directory_conversion() {
    let fixtures = fixtures_dir();
    let output_dir = std::env::temp_dir().join("rd2qmd_test_dir");

    // Clean up
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    let status = Command::new(rd2qmd_binary())
        .arg(&fixtures)
        .arg("-o")
        .arg(&output_dir)
        .arg("-q")
        .status()
        .expect("Failed to run rd2qmd");

    assert!(status.success(), "rd2qmd directory conversion failed");

    // Check that all files were converted
    let mut files: Vec<_> = fs::read_dir(&output_dir)
        .expect("Failed to read output dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    files.sort();

    insta::assert_yaml_snapshot!("directory_files", files);
}
