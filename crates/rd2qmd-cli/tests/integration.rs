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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/rd2qmd")
}

/// Run rd2qmd on a fixture file and return the output
fn convert_fixture(name: &str, args: &[&str]) -> String {
    let input = fixtures_dir().join(format!("{}.Rd", name));
    // Use a unique temp file for each invocation to avoid race conditions
    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let ext = if args.contains(&"md") { "md" } else { "qmd" };
    let output = std::env::temp_dir().join(format!(
        "rd2qmd_test_{}_{}_{}_{}.{}",
        name,
        pid,
        unique_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        ext
    ));

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

/// --topic-link-url applies to both internal and external links when other
/// resolution fails (--no-unresolved-link-url disables the default fallback)
#[test]
fn test_with_links_topic_link_url() {
    let output = convert_fixture(
        "with_links",
        &[
            "--no-unresolved-link-url",
            "--topic-link-url",
            "x-r-help:{package}/{topic}",
        ],
    );
    insta::assert_snapshot!("with_links_topic_link_url", output);
}

#[test]
fn test_formatting() {
    let output = convert_fixture("formatting", &[]);
    insta::assert_snapshot!("formatting_qmd", output);
}

#[test]
fn test_example_control() {
    let output = convert_fixture("example_control", &[]);
    insta::assert_snapshot!("example_control_qmd", output);
}

#[test]
fn test_examplesif() {
    let output = convert_fixture("examplesif", &[]);
    insta::assert_snapshot!("examplesif_qmd", output);
}

#[test]
fn test_examplesif_md() {
    let output = convert_fixture("examplesif", &["-f", "md"]);
    insta::assert_snapshot!("examplesif_md", output);
}

/// \if{html} is excluded by default; \ifelse{html} (lifecycle badge) always renders
#[test]
fn test_conditionals_default() {
    let output = convert_fixture("conditionals", &["--no-frontmatter"]);
    insta::assert_snapshot!("conditionals_default", output);
}

/// \if{html} is included when --include-html-output is set
#[test]
fn test_conditionals_include_html() {
    let output = convert_fixture(
        "conditionals",
        &["--no-frontmatter", "--include-html-output"],
    );
    insta::assert_snapshot!("conditionals_include_html", output);
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

#[test]
fn test_directory_conversion_pipe_table() {
    let fixtures = fixtures_dir();
    let output_dir = std::env::temp_dir().join("rd2qmd_test_dir_pipe_table");

    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    let status = Command::new(rd2qmd_binary())
        .arg(&fixtures)
        .arg("-o")
        .arg(&output_dir)
        .arg("--arguments-format")
        .arg("pipe-table")
        .arg("-q")
        .status()
        .expect("Failed to run rd2qmd");

    assert!(status.success(), "rd2qmd directory conversion failed");

    let content =
        fs::read_to_string(output_dir.join("simple.qmd")).expect("Failed to read simple.qmd");
    let _ = fs::remove_dir_all(&output_dir);

    insta::assert_snapshot!("directory_pipe_table_simple", content);
}

#[test]
fn test_init_config() {
    let output_file = std::env::temp_dir().join("rd2qmd_test_init_config.toml");
    let _ = fs::remove_file(&output_file);

    let status = Command::new(rd2qmd_binary())
        .arg("init")
        .arg("-o")
        .arg(&output_file)
        .status()
        .expect("Failed to run rd2qmd init");

    assert!(status.success(), "rd2qmd init failed");

    let content = fs::read_to_string(&output_file).expect("Failed to read config file");
    let _ = fs::remove_file(&output_file);

    insta::assert_snapshot!("init_config_toml", content);
}

#[test]
fn test_simple_pipe_table() {
    let output = convert_fixture("simple", &["--arguments-format", "pipe-table"]);
    insta::assert_snapshot!("simple_pipe_table", output);
}

#[test]
fn test_simple_list_table() {
    let output = convert_fixture("simple", &["--arguments-format", "list-table"]);
    insta::assert_snapshot!("simple_list_table", output);
}

#[test]
fn test_simple_list() {
    let output = convert_fixture("simple", &["--arguments-format", "list"]);
    insta::assert_snapshot!("simple_list", output);
}

#[test]
fn test_init_schema() {
    let output = Command::new(rd2qmd_binary())
        .arg("init")
        .arg("--schema")
        .output()
        .expect("Failed to run rd2qmd init --schema");

    assert!(output.status.success(), "rd2qmd init --schema failed");

    let schema = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    insta::assert_snapshot!("init_schema_json", schema);
}
