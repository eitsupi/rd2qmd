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

/// Build a unique temporary directory path for a test (not created)
fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "rd2qmd_test_{}_{}_{}_{}",
        prefix,
        std::process::id(),
        unique_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ))
}

/// Run rd2qmd on a fixture file and return the output
fn convert_fixture(name: &str, args: &[&str]) -> String {
    let input = fixtures_dir().join(format!("{}.Rd", name));
    // Use a unique temp file for each invocation to avoid race conditions
    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let ext = if args.contains(&"md") {
        "md"
    } else if args.contains(&"typ") {
        "typ"
    } else {
        "qmd"
    };
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
    cmd.arg("convert").arg(&input).arg("-o").arg(&output);
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
fn parser_diagnostics_are_reported_for_convert_and_parse() {
    let root = unique_temp_dir("diagnostics");
    fs::create_dir_all(&root).unwrap();
    let input = root.join("warning.Rd");
    fs::write(
        &input,
        "\\name{warning}\n\\title{Warning}\n\\examples{\n#ifdef unix\n}\nx <- 1\n#endif\ny <- 2\n}",
    )
    .unwrap();

    let output = root.join("warning.qmd");
    let convert = std::process::Command::new(rd2qmd_binary())
        .args(["convert"])
        .arg(&input)
        .args(["-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(convert.status.success());
    let convert_stderr = String::from_utf8_lossy(&convert.stderr);
    assert!(convert_stderr.contains(&format!("{}:", input.display())));
    assert!(convert_stderr.contains("Warning[UnexpectedClosingDelimiter]"));

    let ast = root.join("warning.json");
    let parse = std::process::Command::new(rd2qmd_binary())
        .args(["parse"])
        .arg(&input)
        .args(["-o"])
        .arg(&ast)
        .output()
        .unwrap();
    assert!(parse.status.success());
    let parse_stderr = String::from_utf8_lossy(&parse.stderr);
    assert!(parse_stderr.contains(&format!("{}:", input.display())));
    assert!(parse_stderr.contains("Warning[UnexpectedClosingDelimiter]"));

    let quiet = std::process::Command::new(rd2qmd_binary())
        .args(["-q", "convert"])
        .arg(&input)
        .args(["-o"])
        .arg(root.join("quiet.qmd"))
        .output()
        .unwrap();
    assert!(quiet.status.success());
    assert!(!String::from_utf8_lossy(&quiet.stderr).contains("UnexpectedClosingDelimiter"));

    let _ = fs::remove_dir_all(root);
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

/// A prose line wrap can produce adjacent `rd_ast::Text` nodes with
/// whitespace on both sides of the node boundary. The core converter must
/// collapse that boundary to one prose space after the real parser path.
#[test]
fn test_issue_54_adjacent_prose_text_nodes() {
    let output = convert_fixture("issue_54", &["--no-frontmatter"]);
    insta::assert_snapshot!("issue_54_qmd", output);
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

/// `\method`/`\S3method`/`\S4method` usage-block parsing through the real
/// parser: S3/S4/default method header comments, multi-signature S4
/// signatures, a mixed usage block combining a plain signature with method
/// variants, special class names, and operator generics (including a
/// user-defined `%...%` infix) reformatted as natural infix expressions.
#[test]
fn test_methods_usage() {
    let output = convert_fixture("methods", &[]);
    insta::assert_snapshot!("methods_qmd", output);
}

/// Rich content nested inside `\arguments{}` item descriptions -- nested
/// `\itemize`/`\describe`/`\tabular`, `\preformatted` (including as an
/// item's sole content), backtick-escaping in `\code{}` and item labels,
/// `\cr` line breaks with list-marker-lookalike continuations, and
/// multi-paragraph descriptions -- rendered with `--arguments-format
/// list-table` (the CLI default).
#[test]
fn test_arguments_rich_list_table() {
    let output = convert_fixture("arguments_rich", &["--arguments-format", "list-table"]);
    insta::assert_snapshot!("arguments_rich_list_table", output);
}

/// Same rich `\arguments{}` content as `test_arguments_rich_list_table`,
/// rendered with `--arguments-format grid-table` -- previously exercised
/// nowhere end-to-end.
#[test]
fn test_arguments_rich_grid_table() {
    let output = convert_fixture("arguments_rich", &["--arguments-format", "grid-table"]);
    insta::assert_snapshot!("arguments_rich_grid_table", output);
}

/// Same rich `\arguments{}` content, rendered with `--arguments-format list`.
#[test]
fn test_arguments_rich_list() {
    let output = convert_fixture("arguments_rich", &["--arguments-format", "list"]);
    insta::assert_snapshot!("arguments_rich_list", output);
}

/// Same rich `\arguments{}` content, rendered with `--arguments-format
/// pipe-table`.
#[test]
fn test_arguments_rich_pipe_table() {
    let output = convert_fixture("arguments_rich", &["--arguments-format", "pipe-table"]);
    insta::assert_snapshot!("arguments_rich_pipe_table", output);
}

/// Tags with no coverage in any other fixture: `\doi`, `\linkS4class`
/// (unqualified and qualified), `\cite`, `\abbr`, `\dontdiff` inside
/// `\examples`, `\code{\link[=...]{...}}` (an explicit-destination link
/// nested inside `\code`, which must preserve the link), and
/// `\link[pkg:topic]{...}` (qualified pkg:topic packed into the bracket).
/// The `\title` also nests `\linkS4class`/`\doi` to guard against tag
/// markup leaking into the frontmatter `title:` value (regression: PR #49).
#[test]
fn test_tags() {
    let output = convert_fixture("tags", &[]);
    insta::assert_snapshot!("tags_qmd", output);
}

/// `\figure{file}{alt text}`, `\figure{file}` (no second argument), and
/// `\figure{file}{options: ...}` (expert form with no `alt=` key) all
/// appear in the `tags` fixture's `\arguments`/`\value` sections; this test
/// exists mainly as documentation pointing at `test_tags`'s snapshot, which
/// covers all three `\figure` forms.
#[test]
fn test_tags_figure_alt_text_forms() {
    let output = convert_fixture("tags", &["--no-frontmatter"]);
    assert!(output.contains("![alt text here](myplot.png)"));
    assert!(output.contains("![myplot.png](myplot.png)"));
}

/// Roxygen2 fenced-code-block markup
/// (`\if{html}{\out{<div class="sourceCode LANG">}}\preformatted{...}\if{html}{\out{</div>}}`)
/// actually converted to Quarto code fences -- an R block, a Python block,
/// a block with no language tag, and a block whose content contains
/// backtick runs (verifying the fence is lengthened past any collision).
/// The `crates/rd2qmd-source` fixture this is adapted from only checks that
/// parsing succeeds without diagnostics; this checks the actual conversion.
#[test]
fn test_roxygen_code_blocks() {
    let output = convert_fixture("roxygen_code_blocks", &["--no-frontmatter"]);
    insta::assert_snapshot!("roxygen_code_blocks_qmd", output);
}

/// `\describe{}` item descriptions containing multi-line block children --
/// a roxygen2 fenced code block (`\if{html}{\out{<div ...>}}\preformatted{...}\if{html}{\out{</div>}}`),
/// a nested `\describe{}` (the `\strong{Arguments}` pattern), and an
/// `\itemize{}` whose item itself contains a `\preformatted{}` block --
/// the layout Rd authors reach by embedding raw markup in a free-text
/// roxygen2 field, as ggplot2 does for its base classes. Regression coverage
/// for continuation-line indentation in the Pandoc definition-list writer:
/// every fence/body/closing-fence line of a block child must be indented,
/// not just the opening fence.
#[test]
fn test_describe_code_blocks() {
    let output = convert_fixture("describe_code_blocks", &["--no-frontmatter"]);
    insta::assert_snapshot!("describe_code_blocks_qmd", output);
}

#[test]
fn test_directory_conversion() {
    let fixtures = fixtures_dir();
    let output_dir = std::env::temp_dir().join("rd2qmd_test_dir");

    // Clean up
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    let status = Command::new(rd2qmd_binary())
        .arg("convert")
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
        .arg("convert")
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
fn test_init_config_quiet() {
    let output_file = std::env::temp_dir().join("rd2qmd_test_init_config_quiet.toml");
    let _ = fs::remove_file(&output_file);

    let output = Command::new(rd2qmd_binary())
        .arg("init")
        .arg("-q")
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("Failed to run rd2qmd init -q");

    assert!(output.status.success(), "rd2qmd init -q failed");
    assert!(
        output.stderr.is_empty(),
        "rd2qmd init -q should not print to stderr, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_file.exists(),
        "rd2qmd init -q should create the config file"
    );
    let _ = fs::remove_file(&output_file);
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
        .arg("convert")
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

/// `parse` then `convert --input-format ast` produces byte-identical output
/// to a direct `convert` for a single file
#[test]
fn test_parse_convert_roundtrip_single_file() {
    let direct = convert_fixture("with_links", &[]);

    let root = unique_temp_dir("roundtrip_single");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    let json_path = root.join("with_links.json");
    let status = Command::new(rd2qmd_binary())
        .arg("parse")
        .arg(fixtures_dir().join("with_links.Rd"))
        .arg("-o")
        .arg(&json_path)
        .status()
        .expect("Failed to run rd2qmd parse");
    assert!(
        status.success(),
        "rd2qmd parse failed with status: {}",
        status
    );

    let qmd_path = root.join("with_links.qmd");
    let status = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(&json_path)
        .arg("--input-format")
        .arg("ast")
        .arg("-o")
        .arg(&qmd_path)
        .status()
        .expect("Failed to run rd2qmd convert");
    assert!(
        status.success(),
        "rd2qmd convert --input-format ast failed with status: {}",
        status
    );

    let via_ast = fs::read_to_string(&qmd_path).expect("Failed to read output file");
    let _ = fs::remove_dir_all(&root);

    assert_eq!(direct, via_ast);
}

/// `parse` then `convert --input-format ast` produces byte-identical output
/// to a direct `convert` for a whole directory
#[test]
fn test_parse_convert_roundtrip_directory() {
    let fixtures = fixtures_dir();
    let root = unique_temp_dir("roundtrip_dir");
    let ast_dir = root.join("ast");
    let out_dir = root.join("out");
    let direct_dir = root.join("direct");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    let status = Command::new(rd2qmd_binary())
        .arg("parse")
        .arg(&fixtures)
        .arg("-o")
        .arg(&ast_dir)
        .status()
        .expect("Failed to run rd2qmd parse");
    assert!(
        status.success(),
        "rd2qmd parse failed with status: {}",
        status
    );

    let status = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(&ast_dir)
        .arg("--input-format")
        .arg("ast")
        .arg("-o")
        .arg(&out_dir)
        .arg("-q")
        .status()
        .expect("Failed to run rd2qmd convert");
    assert!(
        status.success(),
        "rd2qmd convert --input-format ast failed with status: {}",
        status
    );

    let status = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(&fixtures)
        .arg("-o")
        .arg(&direct_dir)
        .arg("-q")
        .status()
        .expect("Failed to run rd2qmd convert");
    assert!(
        status.success(),
        "rd2qmd convert failed with status: {}",
        status
    );

    let mut direct_files: Vec<_> = fs::read_dir(&direct_dir)
        .expect("Failed to read direct output dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    direct_files.sort();
    assert!(!direct_files.is_empty());

    for file in &direct_files {
        let direct_content =
            fs::read_to_string(direct_dir.join(file)).expect("Failed to read direct output file");
        let via_ast_content = fs::read_to_string(out_dir.join(file))
            .unwrap_or_else(|_| panic!("Missing AST-converted output file: {}", file));
        assert_eq!(direct_content, via_ast_content, "Mismatch for {}", file);
    }

    let _ = fs::remove_dir_all(&root);
}

/// `sourceFiles` recorded from a roxygen2 header comment survive a
/// `parse` -> edit -> `convert --input-format ast` round trip and show up in
/// the output frontmatter, same as a direct convert would produce
#[test]
fn test_parse_convert_roundtrip_preserves_source_files() {
    let root = unique_temp_dir("source_files");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    fs::write(
        root.join("roundtrip.Rd"),
        "% Generated by roxygen2: do not edit by hand\n\
         % Please edit documentation in R/roundtrip.R\n\
         \\name{roundtrip}\n\
         \\alias{roundtrip}\n\
         \\title{Roundtrip}\n\
         \\description{\nA function for round-trip testing.\n}\n",
    )
    .expect("Failed to write Rd fixture");

    let json_path = root.join("roundtrip.json");
    let status = Command::new(rd2qmd_binary())
        .arg("parse")
        .arg(root.join("roundtrip.Rd"))
        .arg("-o")
        .arg(&json_path)
        .status()
        .expect("Failed to run rd2qmd parse");
    assert!(
        status.success(),
        "rd2qmd parse failed with status: {}",
        status
    );

    let json = fs::read_to_string(&json_path).expect("Failed to read AST JSON");
    assert!(json.contains("\"sourceFiles\""));
    assert!(json.contains("R/roundtrip.R"));

    let qmd_path = root.join("roundtrip.qmd");
    let status = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(&json_path)
        .arg("--input-format")
        .arg("ast")
        .arg("-o")
        .arg(&qmd_path)
        .status()
        .expect("Failed to run rd2qmd convert");
    assert!(
        status.success(),
        "rd2qmd convert --input-format ast failed with status: {}",
        status
    );

    let qmd = fs::read_to_string(&qmd_path).expect("Failed to read output file");
    let _ = fs::remove_dir_all(&root);

    assert!(qmd.contains("source-files:"));
    assert!(qmd.contains("R/roundtrip.R"));
}

/// An envelope's own `sourceFiles` field is authoritative and must survive
/// `convert --input-format ast` even when it disagrees with (here: is present
/// despite the absence of) a roxygen2 generation-header comment in the
/// document itself -- regression test for roborev job 239 finding 2, where
/// `envelope.source_files` was silently dropped in favor of AST-derived
/// extraction alone.
#[test]
fn test_convert_ast_prefers_envelope_source_files_over_ast_derived() {
    let root = unique_temp_dir("envelope_source_files");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    fs::write(
        root.join("no_header.Rd"),
        "\\name{no_header}\n\
         \\alias{no_header}\n\
         \\title{No Header}\n\
         \\description{\nNo roxygen2 generation-header comment here.\n}\n",
    )
    .expect("Failed to write Rd fixture");

    let json_path = root.join("no_header.json");
    let status = Command::new(rd2qmd_binary())
        .arg("parse")
        .arg(root.join("no_header.Rd"))
        .arg("-o")
        .arg(&json_path)
        .status()
        .expect("Failed to run rd2qmd parse");
    assert!(
        status.success(),
        "rd2qmd parse failed with status: {status}"
    );

    let json = fs::read_to_string(&json_path).expect("Failed to read AST JSON");
    assert!(
        json.contains("\"sourceFiles\": []"),
        "expected no AST-derived source files without a generation header, got: {json}"
    );
    let json = json.replacen(
        "\"sourceFiles\": []",
        "\"sourceFiles\": [\"R/explicit-only.R\"]",
        1,
    );
    fs::write(&json_path, json).expect("Failed to rewrite envelope JSON");

    let qmd_path = root.join("no_header.qmd");
    let status = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(&json_path)
        .arg("--input-format")
        .arg("ast")
        .arg("-o")
        .arg(&qmd_path)
        .status()
        .expect("Failed to run rd2qmd convert");
    assert!(
        status.success(),
        "rd2qmd convert --input-format ast failed with status: {status}"
    );

    let qmd = fs::read_to_string(&qmd_path).expect("Failed to read output file");
    let _ = fs::remove_dir_all(&root);

    assert!(qmd.contains("source-files:"));
    assert!(qmd.contains("R/explicit-only.R"));
}

/// A hand-written envelope with a mismatched `version` field fails
/// `convert --input-format ast` with a non-zero exit and a message
/// mentioning the version
#[test]
fn test_convert_ast_version_mismatch() {
    let root = unique_temp_dir("version_mismatch");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    let json_path = root.join("bad.json");
    fs::write(
        &json_path,
        r#"{"version":99,"source":"bad.Rd","sourceFiles":[],"document":{"sections":[]}}"#,
    )
    .expect("Failed to write bad envelope");

    let output = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(&json_path)
        .arg("--input-format")
        .arg("ast")
        .output()
        .expect("Failed to run rd2qmd convert");

    let _ = fs::remove_dir_all(&root);

    assert!(
        !output.status.success(),
        "rd2qmd convert should fail on a version mismatch"
    );
    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8");
    assert!(
        stderr.contains("version"),
        "Expected a version-mismatch message, got: {}",
        stderr
    );
}

/// Parsing, then rewriting a text node inside the AST JSON, then converting
/// back should surface the rewritten text in the Markdown output -- the
/// motivating use case for AST JSON I/O
#[test]
fn test_parse_edit_convert_pipeline() {
    let root = unique_temp_dir("pipeline");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    let json_path = root.join("with_links.json");
    let status = Command::new(rd2qmd_binary())
        .arg("parse")
        .arg(fixtures_dir().join("with_links.Rd"))
        .arg("-o")
        .arg(&json_path)
        .status()
        .expect("Failed to run rd2qmd parse");
    assert!(
        status.success(),
        "rd2qmd parse failed with status: {}",
        status
    );

    let json = fs::read_to_string(&json_path).expect("Failed to read AST JSON");
    assert!(json.contains("with_links(data)"));
    let rewritten = json.replace("with_links(data)", "with_links(data, verbose = TRUE)");
    fs::write(&json_path, rewritten).expect("Failed to rewrite AST JSON");

    let qmd_path = root.join("with_links.qmd");
    let status = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(&json_path)
        .arg("--input-format")
        .arg("ast")
        .arg("-o")
        .arg(&qmd_path)
        .status()
        .expect("Failed to run rd2qmd convert");
    assert!(
        status.success(),
        "rd2qmd convert --input-format ast failed with status: {}",
        status
    );

    let qmd = fs::read_to_string(&qmd_path).expect("Failed to read output file");
    let _ = fs::remove_dir_all(&root);

    assert!(qmd.contains("with_links(data, verbose = TRUE)"));
}

/// Single-file `parse` with no `-o` writes `<stem>.json` next to the input
#[test]
fn test_parse_defaults_single_file() {
    let root = unique_temp_dir("parse_defaults_single");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    let input = root.join("simple.Rd");
    fs::copy(fixtures_dir().join("simple.Rd"), &input).expect("Failed to copy fixture");

    let status = Command::new(rd2qmd_binary())
        .arg("parse")
        .arg(&input)
        .status()
        .expect("Failed to run rd2qmd parse");
    assert!(
        status.success(),
        "rd2qmd parse failed with status: {}",
        status
    );

    let expected_output = root.join("simple.json");
    assert!(
        expected_output.exists(),
        "Expected {} to exist",
        expected_output.display()
    );
    let json = fs::read_to_string(&expected_output).expect("Failed to read AST JSON");
    let _ = fs::remove_dir_all(&root);

    assert!(json.contains("\"version\": 2"));
}

/// Directory `parse` mirrors input file names with a `.json` extension
#[test]
fn test_parse_defaults_directory() {
    let fixtures = fixtures_dir();
    let root = unique_temp_dir("parse_defaults_dir");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    let status = Command::new(rd2qmd_binary())
        .arg("parse")
        .arg(&fixtures)
        .arg("-o")
        .arg(&root)
        .status()
        .expect("Failed to run rd2qmd parse");
    assert!(
        status.success(),
        "rd2qmd parse failed with status: {}",
        status
    );

    let mut files: Vec<_> = fs::read_dir(&root)
        .expect("Failed to read output dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    files.sort();

    let mut expected: Vec<_> = fs::read_dir(&fixtures)
        .expect("Failed to read fixtures dir")
        .filter_map(|e| e.ok())
        .map(|e| {
            PathBuf::from(e.file_name())
                .with_extension("json")
                .to_string_lossy()
                .to_string()
        })
        .collect();
    expected.sort();

    let _ = fs::remove_dir_all(&root);

    assert_eq!(files, expected);
}

#[test]
fn test_convert_empty_ast_directory_mentions_json_files() {
    let root = unique_temp_dir("convert_empty_ast");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    let output = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(&root)
        .arg("--input-format")
        .arg("ast")
        .output()
        .expect("Failed to run rd2qmd convert");

    let _ = fs::remove_dir_all(&root);

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No .json files found"));
}

#[test]
fn test_index_empty_ast_directory_mentions_json_files() {
    let root = unique_temp_dir("index_empty_ast");
    fs::create_dir_all(&root).expect("Failed to create temp dir");

    let output = Command::new(rd2qmd_binary())
        .arg("index")
        .arg(&root)
        .arg("--input-format")
        .arg("ast")
        .output()
        .expect("Failed to run rd2qmd index");

    let _ = fs::remove_dir_all(&root);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No .json files found"));
}

#[test]
fn test_simple_to_typst() {
    let output = convert_fixture("simple", &["-f", "typ"]);
    insta::assert_snapshot!("simple_typ", output);
}

#[test]
fn test_arguments_rich_to_typst() {
    let output = convert_fixture("arguments_rich", &["-f", "typ"]);
    insta::assert_snapshot!("arguments_rich_typ", output);
}

#[test]
fn test_typst_directory_links_resolve_to_typ_files() {
    let output_dir = unique_temp_dir("typst_dir");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    let status = Command::new(rd2qmd_binary())
        .arg("convert")
        .arg(fixtures_dir())
        .arg("-o")
        .arg(&output_dir)
        .args(["-f", "typ", "-q", "--no-external-links"])
        .status()
        .expect("Failed to run rd2qmd");
    assert!(status.success(), "rd2qmd directory conversion failed");

    let converted = fs::read_to_string(output_dir.join("with_links.typ")).expect("with_links.typ");
    // Alias-resolved internal links follow the output extension, like every
    // other format.
    assert!(
        converted.contains("#link(\"simple.typ\")"),
        "expected an internal link to a .typ file, got:\n{converted}"
    );

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_typst_examples_are_plain_r_blocks() {
    let output = convert_fixture("example_control", &["-f", "typ"]);
    // Never `{r}`: calepin executes a plain raw block, plain typst renders it.
    assert!(output.contains("```r\n"), "expected a plain ```r block");
    assert!(
        !output.contains("{r}"),
        "expected no Quarto executable block"
    );
}
