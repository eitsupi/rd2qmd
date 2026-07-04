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

/// Defaults: --external-link-url and --unqualified-link-url are applied even
/// when not passed explicitly
#[test]
fn test_with_links() {
    let output = convert_fixture("with_links", &[]);
    insta::assert_snapshot!("with_links_qmd", output);
}

/// --no-external-link-url and --no-unqualified-link-url disable the fallback
/// templates: unresolvable links degrade to plain inline code
#[test]
fn test_with_links_no_link_fallbacks() {
    let output = convert_fixture(
        "with_links",
        &["--no-external-link-url", "--no-unqualified-link-url"],
    );
    insta::assert_snapshot!("with_links_no_link_fallbacks", output);
}

/// Custom templates: qualified links use --external-link-url, unqualified
/// links use --unqualified-link-url
#[test]
fn test_with_links_custom_templates() {
    let output = convert_fixture(
        "with_links",
        &[
            "--external-link-url",
            "x-r-help:{package}/{topic}",
            "--unqualified-link-url",
            "x-r-help:{topic}",
        ],
    );
    insta::assert_snapshot!("with_links_custom_templates", output);
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

/// Run rd2qmd in directory mode with external link resolution against an
/// empty R library and return the captured stderr. The fixture links to
/// `dplyr` (covered by `[links.package_urls]` in the config, so it must not
/// be reported as a fallback) and to `somepkg` (unresolvable).
fn external_links_warnings(extra_args: &[&str]) -> String {
    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!(
        "rd2qmd_extlinks_{}_{}_{}",
        std::process::id(),
        unique_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let man_dir = root.join("man");
    let lib_dir = root.join("emptylib");
    fs::create_dir_all(&man_dir).expect("Failed to create man dir");
    fs::create_dir_all(&lib_dir).expect("Failed to create lib dir");

    fs::write(
        man_dir.join("alpha.Rd"),
        "\\name{alpha}\n\\alias{alpha}\n\\title{Alpha}\n\\description{Uses \\link[dplyr]{mutate} and \\link[somepkg]{thing}.}\n",
    )
    .expect("Failed to write Rd fixture");

    let config_path = root.join("_rd2qmd.toml");
    fs::write(
        &config_path,
        "[links.package_urls]\ndplyr = \"https://dplyr.tidyverse.org/reference/{topic}.html\"\n",
    )
    .expect("Failed to write config");

    let output = Command::new(rd2qmd_binary())
        .arg(&man_dir)
        .arg("-o")
        .arg(root.join("out"))
        .arg("--config")
        .arg(&config_path)
        .arg("--r-lib-path")
        .arg(&lib_dir)
        .args(extra_args)
        .output()
        .expect("Failed to run rd2qmd");
    assert!(
        output.status.success(),
        "rd2qmd failed with status: {}",
        output.status
    );

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8");
    let _ = fs::remove_dir_all(&root);
    stderr
}

/// Unresolvable packages are warned about with the actual outcome; packages
/// covered by user-provided package_urls are excluded from resolution
#[test]
fn test_external_links_fallback_warning() {
    let stderr = external_links_warnings(&[]);
    insta::assert_snapshot!("external_links_fallback_warning", stderr);
}

/// With --no-external-link-url the warning reports that links degrade to
/// plain inline code instead of claiming the disabled fallback will be used
#[test]
fn test_external_links_fallback_warning_no_external_link_url() {
    let stderr = external_links_warnings(&["--no-external-link-url"]);
    insta::assert_snapshot!(
        "external_links_fallback_warning_no_external_link_url",
        stderr
    );
}
