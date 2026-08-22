use super::*;
use std::fs;
use tempfile::tempdir;

use crate::convert::{
    ConvertOutcome, convert_single_file, export_single_file, has_keyword_internal,
};
use crate::package::build_alias_index;
use rd2qmd_core::{ArgumentsFormat, RdAstEnvelope, RdMetadata};

#[test]
fn test_build_alias_index() {
    let dir = tempdir().unwrap();

    // Create a test Rd file
    let rd_content = r#"\name{my_func}
\alias{my_func}
\alias{my_func_alias}
\title{My Function}
\description{A test function}
"#;
    let rd_path = dir.path().join("my_func.Rd");
    fs::write(&rd_path, rd_content).unwrap();

    let files = vec![rd_path];
    let index = build_alias_index(&files, InputFormat::Rd).unwrap();

    assert_eq!(index.get("my_func"), Some(&"my_func".to_string()));
    assert_eq!(index.get("my_func_alias"), Some(&"my_func".to_string()));
}

#[test]
fn test_rd_package_from_directory() {
    let dir = tempdir().unwrap();

    // Create test Rd files
    let rd1 = r#"\name{func_a}
\alias{func_a}
\alias{FuncA}
\title{Function A}
"#;
    let rd2 = r#"\name{func_b}
\alias{func_b}
\title{Function B}
"#;
    fs::write(dir.path().join("func_a.Rd"), rd1).unwrap();
    fs::write(dir.path().join("func_b.Rd"), rd2).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();

    assert_eq!(package.files.len(), 2);
    assert_eq!(package.resolve_alias("func_a"), Some("func_a"));
    assert_eq!(package.resolve_alias("FuncA"), Some("func_a"));
    assert_eq!(package.resolve_alias("func_b"), Some("func_b"));
    assert_eq!(package.resolve_alias("nonexistent"), None);
}

#[test]
fn test_generate_topic_index() {
    let dir = tempdir().unwrap();

    // Create test Rd files - one with lifecycle, one without
    let rd_deprecated = r#"\name{old_func}
\alias{old_func}
\alias{legacy_func}
\title{Old Function}
\description{
\ifelse{html}{\href{https://lifecycle.r-lib.org/}{\figure{lifecycle-deprecated.svg}{}}}{\strong{[Deprecated]}}
An old deprecated function.
}
"#;
    let rd_normal = r#"\name{new_func}
\alias{new_func}
\title{New Function}
\description{A normal function.}
"#;
    fs::write(dir.path().join("old_func.Rd"), rd_deprecated).unwrap();
    fs::write(dir.path().join("new_func.Rd"), rd_normal).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = TopicIndexOptions {
        output_extension: "qmd".to_string(),
        include_internal: false,
    };
    let index = generate_topic_index(&package, &options).unwrap();

    assert_eq!(index.topics.len(), 2);

    // Topics are sorted by name
    let new_topic = index.topics.iter().find(|t| t.name == "new_func").unwrap();
    assert_eq!(new_topic.file, "new_func.qmd");
    assert_eq!(new_topic.title, "New Function");
    assert!(new_topic.metadata.aliases.contains(&"new_func".to_string()));
    assert!(new_topic.metadata.lifecycle.is_none());

    let old_topic = index.topics.iter().find(|t| t.name == "old_func").unwrap();
    assert_eq!(old_topic.file, "old_func.qmd");
    assert_eq!(old_topic.title, "Old Function");
    assert!(old_topic.metadata.aliases.contains(&"old_func".to_string()));
    assert!(
        old_topic
            .metadata
            .aliases
            .contains(&"legacy_func".to_string())
    );
    assert_eq!(old_topic.metadata.lifecycle, Some("deprecated".to_string()));

    // Both are hand-written, so no source_files
    assert!(new_topic.metadata.source_files.is_empty());
    assert!(old_topic.metadata.source_files.is_empty());
}

#[test]
fn test_package_results_retain_parser_diagnostics() {
    let dir = tempdir().unwrap();
    let content = r#"\name{warning_topic}
\title{Warning topic}
\examples{
#ifdef unix
}
x <- 1
#endif
y <- 2
}"#;
    let input = dir.path().join("warning_topic.Rd");
    fs::write(&input, content).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: dir.path().join("out"),
        output_extension: "qmd".to_string(),
        ..Default::default()
    };
    let conversion = convert_package(&package, &options).unwrap();
    assert_eq!(conversion.diagnostics.len(), 1);
    assert_eq!(conversion.diagnostics[0].file, input);
    assert!(!conversion.diagnostics[0].diagnostics.is_empty());

    let index = generate_topic_index_with_diagnostics(
        &package,
        &TopicIndexOptions {
            output_extension: "qmd".to_string(),
            include_internal: false,
        },
    )
    .unwrap();
    assert_eq!(index.diagnostics.len(), 1);
    assert_eq!(index.diagnostics[0].file, input);
}

#[test]
fn test_convert_retains_diagnostics_when_output_write_fails() {
    let dir = tempdir().unwrap();
    let content = r#"\name{warning_topic}
\title{Warning topic}
\examples{
#ifdef unix
}
x <- 1
#endif
y <- 2
}"#;
    let input = dir.path().join("warning_topic.Rd");
    fs::write(&input, content).unwrap();

    // Force the output write to fail after parsing succeeds: make the
    // output directory a plain file, so create_dir_all cannot create it.
    let output_dir = dir.path().join("blocker");
    fs::write(&output_dir, "not a directory").unwrap();

    let options = PackageConvertOptions {
        output_dir,
        output_extension: "qmd".to_string(),
        ..Default::default()
    };
    let package = RdPackage::from_directory(dir.path(), false).unwrap();

    // convert_package's own fs::create_dir_all(&options.output_dir) call
    // fails first in this setup, so exercise the per-file path directly.
    let outcome = convert_single_file(&input, &package, &options);
    match outcome {
        ConvertOutcome::Failed(path, _message, diagnostics) => {
            assert_eq!(path, input);
            assert!(
                !diagnostics.is_empty(),
                "expected parser warnings to survive a post-parse write failure"
            );
        }
        _ => panic!("expected the conversion to fail due to the blocked output directory"),
    }
}

#[test]
fn test_export_results_retain_parser_diagnostics() {
    let dir = tempdir().unwrap();
    let content = r#"\name{warning_topic}
\title{Warning topic}
\examples{
#ifdef unix
}
#endif
}"#;
    let input = dir.path().join("warning_topic.Rd");
    fs::write(&input, content).unwrap();

    let output = dir.path().join("ast");
    let result = export_package_ast(dir.path(), false, &output, Some(1)).unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].file, input);
}

#[test]
fn test_export_retains_diagnostics_when_output_write_fails() {
    let dir = tempdir().unwrap();
    let content = r#"\name{warning_topic}
\title{Warning topic}
\examples{
#ifdef unix
}
#endif
}"#;
    let input = dir.path().join("warning_topic.Rd");
    fs::write(&input, content).unwrap();

    // Force the JSON write to fail after parsing succeeds: make the
    // output directory a plain file, so create_dir_all cannot create it.
    let output_dir = dir.path().join("blocker");
    fs::write(&output_dir, "not a directory").unwrap();

    let outcome = export_single_file(&input, dir.path(), &output_dir);
    match outcome {
        Err((path, _message, diagnostics)) => {
            assert_eq!(path, input);
            assert!(
                !diagnostics.is_empty(),
                "expected parser warnings to survive a post-parse write failure"
            );
        }
        Ok(_) => panic!("expected the export to fail due to the blocked output directory"),
    }
}

#[test]
fn test_generate_topic_index_with_roxygen_sources() {
    let dir = tempdir().unwrap();

    // Create a roxygen2-generated Rd file with source files
    let rd_roxygen = r#"% Generated by roxygen2: do not edit by hand
% Please edit documentation in R/coord-map.R, R/coord-quickmap.R
\name{coord_map}
\alias{coord_map}
\alias{coord_quickmap}
\title{Map projections}
\description{Projects coordinates onto a map.}
"#;
    // Create a hand-written Rd file (no roxygen2 header)
    let rd_manual = r#"\name{manual}
\alias{manual}
\title{Manual Topic}
\description{Hand-written documentation.}
"#;
    fs::write(dir.path().join("coord_map.Rd"), rd_roxygen).unwrap();
    fs::write(dir.path().join("manual.Rd"), rd_manual).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = TopicIndexOptions {
        output_extension: "qmd".to_string(),
        include_internal: false,
    };
    let index = generate_topic_index(&package, &options).unwrap();

    assert_eq!(index.topics.len(), 2);

    // Roxygen-generated topic has source_files
    let coord_topic = index.topics.iter().find(|t| t.name == "coord_map").unwrap();
    assert_eq!(
        coord_topic.metadata.source_files,
        vec!["R/coord-map.R", "R/coord-quickmap.R"]
    );

    // Manual topic has no source_files
    let manual_topic = index.topics.iter().find(|t| t.name == "manual").unwrap();
    assert!(manual_topic.metadata.source_files.is_empty());
}

#[test]
fn test_topic_index_json_serialization() {
    let index = TopicIndex {
        topics: vec![
            TopicInfo {
                name: "foo".to_string(),
                file: "foo.qmd".to_string(),
                title: "Foo Function".to_string(),
                metadata: RdMetadata {
                    lifecycle: Some("deprecated".to_string()),
                    aliases: vec!["foo".to_string(), "bar".to_string()],
                    keywords: vec![],
                    concepts: vec![],
                    source_files: vec!["R/foo.R".to_string(), "R/bar.R".to_string()],
                },
            },
            TopicInfo {
                name: "baz".to_string(),
                file: "baz.qmd".to_string(),
                title: "Baz Function".to_string(),
                metadata: RdMetadata {
                    lifecycle: None,
                    aliases: vec!["baz".to_string()],
                    keywords: vec![],
                    concepts: vec![],
                    source_files: vec![], // Empty - should be omitted from JSON
                },
            },
        ],
    };

    let json = index.to_json().unwrap();

    // Parse JSON to verify structure
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let topics = parsed["topics"].as_array().unwrap();
    assert_eq!(topics.len(), 2);

    // First topic has lifecycle and source_files (flattened from metadata)
    assert_eq!(topics[0]["name"], "foo");
    assert_eq!(topics[0]["lifecycle"], "deprecated");
    assert_eq!(
        topics[0]["source_files"],
        serde_json::json!(["R/foo.R", "R/bar.R"])
    );

    // Second topic has no lifecycle or source_files fields (skip_serializing_if)
    assert_eq!(topics[1]["name"], "baz");
    assert!(topics[1].get("lifecycle").is_none());
    assert!(topics[1].get("source_files").is_none());
}

// ========================================================================
// PackageConverter Builder tests
// ========================================================================

#[test]
fn test_package_converter_basic() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    // Create test Rd files
    let rd1 = r#"\name{alpha}
\alias{alpha}
\title{Alpha Function}
\description{The alpha function.}
"#;
    let rd2 = r#"\name{beta}
\alias{beta}
\title{Beta Function}
\description{The beta function.}
"#;
    fs::write(dir.path().join("alpha.Rd"), rd1).unwrap();
    fs::write(dir.path().join("beta.Rd"), rd2).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: true,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: None,
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();

    assert_eq!(result.conversion.success_count, 2);
    assert!(result.conversion.failed_files.is_empty());
    assert_eq!(result.conversion.output_files.len(), 2);

    // Check output files exist
    assert!(out_dir.path().join("alpha.qmd").exists());
    assert!(out_dir.path().join("beta.qmd").exists());

    // Check content
    let alpha_content = fs::read_to_string(out_dir.path().join("alpha.qmd")).unwrap();
    assert!(alpha_content.contains("title: \"Alpha Function\""));
    assert!(!alpha_content.contains("\n# Alpha Function\n"));

    // Fallbacks should be empty when external links not used
    #[cfg(feature = "external-links")]
    assert!(result.fallbacks.is_empty());
}

#[test]
fn test_package_converter_with_alias_resolution() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    // Create Rd files that reference each other
    let rd_main = r#"\name{main_func}
\alias{main_func}
\alias{mf}
\title{Main Function}
\description{See \link{helper_func} for details.}
"#;
    let rd_helper = r#"\name{helper_func}
\alias{helper_func}
\title{Helper Function}
\description{A helper for \link{mf}.}
"#;
    fs::write(dir.path().join("main_func.Rd"), rd_main).unwrap();
    fs::write(dir.path().join("helper_func.Rd"), rd_helper).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: false,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: None,
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();
    assert_eq!(result.conversion.success_count, 2);

    // Check alias resolution works (links use [`text`](url) format)
    let main_content = fs::read_to_string(out_dir.path().join("main_func.qmd")).unwrap();
    assert!(main_content.contains("[`helper_func`](helper_func.qmd)"));

    let helper_content = fs::read_to_string(out_dir.path().join("helper_func.qmd")).unwrap();
    // "mf" alias should resolve to main_func
    assert!(helper_content.contains("[`mf`](main_func.qmd)"));
}

#[test]
fn test_package_converter_md_output() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd = r#"\name{test}
\alias{test}
\title{Test}
\description{Test function.}
\examples{
x <- 1
}
"#;
    fs::write(dir.path().join("test.Rd"), rd).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "md".to_string(),
        frontmatter: true,
        pagetitle: true,
        quarto_code_blocks: false, // Plain markdown
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: None,
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();
    assert_eq!(result.conversion.success_count, 1);

    // Check .md extension
    assert!(out_dir.path().join("test.md").exists());

    let content = fs::read_to_string(out_dir.path().join("test.md")).unwrap();
    // Should have pagetitle
    assert!(content.contains("pagetitle: \"Test — test\""));
    // Should use plain code blocks, not {r}
    assert!(content.contains("```r"));
    assert!(!content.contains("```{r}"));
}

#[test]
fn test_package_converter_handles_parse_errors_at_load_time() {
    let dir = tempdir().unwrap();

    // One valid file
    let rd_good = r#"\name{good}
\alias{good}
\title{Good}
\description{Works fine.}
"#;
    // One invalid file (unclosed brace)
    let rd_bad = r#"\name{bad
\title{Bad}
"#;
    fs::write(dir.path().join("good.Rd"), rd_good).unwrap();
    fs::write(dir.path().join("bad.Rd"), rd_bad).unwrap();

    // from_directory fails when any file has parse errors (during alias index building)
    let result = RdPackage::from_directory(dir.path(), false);
    assert!(result.is_err());

    // The error should be a parse error
    let err = result.unwrap_err();
    assert!(err.to_string().contains("bad.Rd"));
}

#[test]
fn test_package_converter_with_unqualified_link_url() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd = r#"\name{caller}
\alias{caller}
\title{Caller}
\description{Uses \link{unknown_external}.}
"#;
    fs::write(dir.path().join("caller.Rd"), rd).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: false,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: Some("https://rdrr.io/r/base/{topic}.html".to_string()),
        package_urls: None,
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();
    assert_eq!(result.conversion.success_count, 1);

    let content = fs::read_to_string(out_dir.path().join("caller.qmd")).unwrap();
    // Link text has backticks
    assert!(content.contains("[`unknown_external`](https://rdrr.io/r/base/unknown_external.html)"));
}

#[test]
fn test_package_converter_with_external_link_url() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd = r#"\name{caller}
\alias{caller}
\title{Caller}
\description{Uses \link{caller} and \link[somepkg]{something}.}
"#;
    fs::write(dir.path().join("caller.Rd"), rd).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: false,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: None,
        external_link_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();
    assert_eq!(result.conversion.success_count, 1);

    let content = fs::read_to_string(out_dir.path().join("caller.qmd")).unwrap();
    // Alias-resolved internal link uses the derived {file}.qmd template
    assert!(content.contains("[`caller`](caller.qmd)"));
    // Qualified link whose package is not in package_urls falls back to the template
    assert!(
        content.contains("[`somepkg::something`](https://rdrr.io/pkg/somepkg/man/something.html)")
    );
}

#[test]
fn test_package_converter_with_package_urls() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd = r#"\name{tidyverse_user}
\alias{tidyverse_user}
\title{Tidyverse User}
\description{Uses \link[dplyr]{mutate} and \link[ggplot2]{ggplot}.}
"#;
    fs::write(dir.path().join("tidyverse_user.Rd"), rd).unwrap();

    let mut package_urls_map = std::collections::HashMap::new();
    package_urls_map.insert(
        "dplyr".to_string(),
        "https://dplyr.tidyverse.org/reference/{topic}.html".to_string(),
    );
    package_urls_map.insert(
        "ggplot2".to_string(),
        "https://ggplot2.tidyverse.org/reference/{topic}.html".to_string(),
    );

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: false,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: Some(package_urls_map),
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();
    assert_eq!(result.conversion.success_count, 1);

    let content = fs::read_to_string(out_dir.path().join("tidyverse_user.qmd")).unwrap();
    // Link text uses [`package::topic`] format
    assert!(
        content.contains("[`dplyr::mutate`](https://dplyr.tidyverse.org/reference/mutate.html)")
    );
    assert!(
        content
            .contains("[`ggplot2::ggplot`](https://ggplot2.tidyverse.org/reference/ggplot.html)")
    );
}

#[test]
fn test_package_converter_package_urls_win_over_external_link_url() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd = r#"\name{wrapper}
\alias{wrapper}
\title{Wrapper}
\description{Uses \link[dplyr]{mutate} and \link[somepkg]{something}.}
"#;
    fs::write(dir.path().join("wrapper.Rd"), rd).unwrap();

    let mut package_urls_map = std::collections::HashMap::new();
    package_urls_map.insert(
        "dplyr".to_string(),
        "https://dplyr.tidyverse.org/reference/{topic}.html".to_string(),
    );

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        frontmatter: false,
        pagetitle: false,
        parallel_jobs: Some(1),
        package_urls: Some(package_urls_map),
        external_link_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
        ..Default::default()
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();
    assert_eq!(result.conversion.success_count, 1);

    let content = fs::read_to_string(out_dir.path().join("wrapper.qmd")).unwrap();
    // package_urls entry takes precedence over external_link_url
    assert!(
        content.contains("[`dplyr::mutate`](https://dplyr.tidyverse.org/reference/mutate.html)")
    );
    // Packages not in package_urls fall back to external_link_url
    assert!(
        content.contains("[`somepkg::something`](https://rdrr.io/pkg/somepkg/man/something.html)")
    );
}

#[cfg(feature = "external-links")]
#[test]
fn test_external_links_skip_user_covered_packages() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap(); // empty: nothing resolvable

    let rd = r#"\name{wrapper}
\alias{wrapper}
\title{Wrapper}
\description{Uses \link[dplyr]{mutate} and \link[somepkg]{something}.}
"#;
    fs::write(dir.path().join("wrapper.Rd"), rd).unwrap();

    let mut package_urls_map = std::collections::HashMap::new();
    package_urls_map.insert(
        "dplyr".to_string(),
        "https://dplyr.tidyverse.org/reference/{topic}.html".to_string(),
    );

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        frontmatter: false,
        pagetitle: false,
        parallel_jobs: Some(1),
        package_urls: Some(package_urls_map),
        external_link_url: Some("https://rdrr.io/pkg/{package}/man/{topic}.html".to_string()),
        ..Default::default()
    };

    let result = PackageConverter::new(&package, options)
        .with_external_links(ExternalLinkOptions {
            lib_paths: vec![lib_dir.path().to_path_buf()],
            cache_dir: None,
        })
        .convert()
        .unwrap();
    assert_eq!(result.conversion.success_count, 1);

    // Packages covered by user-provided package_urls are not sent to the
    // resolver, so they must not show up as fallbacks
    assert!(!result.fallbacks.contains_key("dplyr"));
    assert_eq!(
        result.fallbacks.get("somepkg"),
        Some(&FallbackReason::NotInstalled)
    );

    let content = fs::read_to_string(out_dir.path().join("wrapper.qmd")).unwrap();
    assert!(
        content.contains("[`dplyr::mutate`](https://dplyr.tidyverse.org/reference/mutate.html)")
    );
    assert!(
        content.contains("[`somepkg::something`](https://rdrr.io/pkg/somepkg/man/something.html)")
    );
}

#[test]
fn test_package_converter_internal_link_url_override() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd_main = r#"\name{main_func}
\alias{main_func}
\title{Main Function}
\description{See \link{helper_func} for details.}
"#;
    let rd_helper = r#"\name{helper_func}
\alias{helper_func}
\title{Helper Function}
\description{A helper.}
"#;
    fs::write(dir.path().join("main_func.Rd"), rd_main).unwrap();
    fs::write(dir.path().join("helper_func.Rd"), rd_helper).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        frontmatter: false,
        pagetitle: false,
        parallel_jobs: Some(1),
        // Override the derived {file}.qmd template
        internal_link_url: Some("/reference/{file}.html".to_string()),
        ..Default::default()
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();
    assert_eq!(result.conversion.success_count, 2);

    let content = fs::read_to_string(out_dir.path().join("main_func.qmd")).unwrap();
    assert!(content.contains("[`helper_func`](/reference/helper_func.html)"));
}

#[test]
fn test_package_converter_empty_directory() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    assert!(package.files.is_empty());

    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: false,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: None,
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();

    assert_eq!(result.conversion.success_count, 0);
    assert!(result.conversion.failed_files.is_empty());
    assert!(result.conversion.output_files.is_empty());
}

#[test]
fn test_full_convert_result_structure() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd = r#"\name{simple}
\alias{simple}
\title{Simple}
\description{A simple function.}
"#;
    fs::write(dir.path().join("simple.Rd"), rd).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: false,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: None,
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false,
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();

    // Check FullConvertResult fields
    assert_eq!(result.conversion.success_count, 1);
    assert!(result.conversion.failed_files.is_empty());
    assert_eq!(result.conversion.output_files.len(), 1);
    // Fallbacks are empty when not using external links feature
    #[cfg(feature = "external-links")]
    assert!(result.fallbacks.is_empty());
}

// ========================================================================
// Internal topic skipping tests
// ========================================================================

#[test]
fn test_internal_topics_skipped_by_default() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    // Create one public and one internal topic
    let rd_public = r#"\name{public_func}
\alias{public_func}
\title{Public Function}
\description{A public function.}
"#;
    let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
    fs::write(dir.path().join("public_func.Rd"), rd_public).unwrap();
    fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();
    assert_eq!(package.files.len(), 2);

    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: false,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: None,
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: false, // Default: skip internal
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();

    // Only public topic should be converted
    assert_eq!(result.conversion.success_count, 1);
    assert_eq!(result.conversion.skipped_internal.len(), 1);
    assert!(result.conversion.failed_files.is_empty());

    // Check that only public_func.qmd was created
    assert!(out_dir.path().join("public_func.qmd").exists());
    assert!(!out_dir.path().join("internal_func.qmd").exists());

    // Check skipped file name
    assert!(
        result.conversion.skipped_internal[0]
            .to_string_lossy()
            .contains("internal_func.Rd")
    );
}

#[test]
fn test_internal_topics_included_when_requested() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    // Create one public and one internal topic
    let rd_public = r#"\name{public_func}
\alias{public_func}
\title{Public Function}
\description{A public function.}
"#;
    let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
    fs::write(dir.path().join("public_func.Rd"), rd_public).unwrap();
    fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();

    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        output_extension: "qmd".to_string(),
        frontmatter: false,
        pagetitle: false,
        quarto_code_blocks: true,
        parallel_jobs: Some(1),
        internal_link_url: None,
        unqualified_link_url: None,
        package_urls: None,
        external_link_url: None,
        exec_dontrun: false,
        exec_donttest: true,
        include_internal: true, // Include internal topics
        include_html_output: false,
        prefer_ascii_math: false,
        target: Default::default(),
        arguments_format: ArgumentsFormat::default(),
    };

    let result = PackageConverter::new(&package, options).convert().unwrap();

    // Both topics should be converted
    assert_eq!(result.conversion.success_count, 2);
    assert!(result.conversion.skipped_internal.is_empty());
    assert!(result.conversion.failed_files.is_empty());

    // Check that both files were created
    assert!(out_dir.path().join("public_func.qmd").exists());
    assert!(out_dir.path().join("internal_func.qmd").exists());
}

#[test]
fn test_has_keyword_internal_detection() {
    // Test the has_keyword_internal helper function
    let rd_internal = r#"\name{func}
\keyword{internal}
\title{Test}
"#;
    let rd_normal = r#"\name{func}
\keyword{datasets}
\title{Test}
"#;
    let rd_no_keyword = r#"\name{func}
\title{Test}
"#;

    let doc_internal = rd2qmd_source::parse(rd_internal)
        .unwrap()
        .document()
        .clone();
    let doc_normal = rd2qmd_source::parse(rd_normal).unwrap().document().clone();
    let doc_no_keyword = rd2qmd_source::parse(rd_no_keyword)
        .unwrap()
        .document()
        .clone();

    assert!(has_keyword_internal(&doc_internal));
    assert!(!has_keyword_internal(&doc_normal));
    assert!(!has_keyword_internal(&doc_no_keyword));
}

#[test]
fn test_topic_index_excludes_internal_by_default() {
    let dir = tempdir().unwrap();

    // Create public and internal topics
    let rd_public = r#"\name{public_func}
\alias{public_func}
\title{Public Function}
\description{A public function.}
"#;
    let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
    fs::write(dir.path().join("public_func.Rd"), rd_public).unwrap();
    fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();

    // Default: exclude internal
    let options = TopicIndexOptions {
        output_extension: "qmd".to_string(),
        include_internal: false,
    };
    let index = generate_topic_index(&package, &options).unwrap();

    // Only public topic should be in the index
    assert_eq!(index.topics.len(), 1);
    assert_eq!(index.topics[0].name, "public_func");
}

#[test]
fn test_topic_index_includes_internal_when_requested() {
    let dir = tempdir().unwrap();

    // Create public and internal topics
    let rd_public = r#"\name{public_func}
\alias{public_func}
\title{Public Function}
\description{A public function.}
"#;
    let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
    fs::write(dir.path().join("public_func.Rd"), rd_public).unwrap();
    fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

    let package = RdPackage::from_directory(dir.path(), false).unwrap();

    // Include internal topics
    let options = TopicIndexOptions {
        output_extension: "qmd".to_string(),
        include_internal: true,
    };
    let index = generate_topic_index(&package, &options).unwrap();

    // Both topics should be in the index
    assert_eq!(index.topics.len(), 2);

    // Topics are sorted by name
    let names: Vec<_> = index.topics.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"public_func"));
    assert!(names.contains(&"internal_func"));
}

// ========================================================================
// AST JSON export/import tests
// ========================================================================

#[test]
fn test_export_package_ast() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd_roxygen = r#"% Generated by roxygen2: do not edit by hand
% Please edit documentation in R/coord-map.R
\name{coord_map}
\alias{coord_map}
\title{Map projections}
\description{Projects coordinates onto a map.}
"#;
    fs::write(dir.path().join("coord_map.Rd"), rd_roxygen).unwrap();

    let result = export_package_ast(dir.path(), false, out_dir.path(), Some(1)).unwrap();

    assert_eq!(result.success_count, 1);
    assert!(result.failed_files.is_empty());
    assert!(result.skipped_internal.is_empty());

    let json_path = out_dir.path().join("coord_map.json");
    assert!(json_path.exists());

    let content = fs::read_to_string(&json_path).unwrap();
    let envelope = RdAstEnvelope::from_json(&content).unwrap();
    assert_eq!(envelope.source, Some("coord_map.Rd".to_string()));
    assert_eq!(envelope.source_files, vec!["R/coord-map.R".to_string()]);
}

#[test]
fn test_export_package_ast_exports_internal_topics_too() {
    let dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd_internal = r#"\name{internal_func}
\alias{internal_func}
\title{Internal Function}
\keyword{internal}
\description{An internal function.}
"#;
    fs::write(dir.path().join("internal_func.Rd"), rd_internal).unwrap();

    let result = export_package_ast(dir.path(), false, out_dir.path(), Some(1)).unwrap();

    assert_eq!(result.success_count, 1);
    assert!(out_dir.path().join("internal_func.json").exists());
}

#[test]
fn test_package_from_directory_with_ast_json_format() {
    let dir = tempdir().unwrap();
    let json_dir = tempdir().unwrap();
    let out_dir = tempdir().unwrap();

    let rd_main = r#"\name{main_func}
\alias{main_func}
\title{Main Function}
\description{See \link{helper_func} for details.}
"#;
    let rd_helper = r#"\name{helper_func}
\alias{helper_func}
\title{Helper Function}
\description{A helper function.}
"#;
    fs::write(dir.path().join("main_func.Rd"), rd_main).unwrap();
    fs::write(dir.path().join("helper_func.Rd"), rd_helper).unwrap();

    export_package_ast(dir.path(), false, json_dir.path(), Some(1)).unwrap();

    let package =
        RdPackage::from_directory_with_format(json_dir.path(), false, InputFormat::AstJson)
            .unwrap();
    assert_eq!(package.files.len(), 2);
    assert_eq!(package.resolve_alias("main_func"), Some("main_func"));
    assert_eq!(package.resolve_alias("helper_func"), Some("helper_func"));

    let options = PackageConvertOptions {
        output_dir: out_dir.path().to_path_buf(),
        frontmatter: false,
        pagetitle: false,
        parallel_jobs: Some(1),
        ..Default::default()
    };
    let result = PackageConverter::new(&package, options).convert().unwrap();
    assert_eq!(result.conversion.success_count, 2);

    let main_content = fs::read_to_string(out_dir.path().join("main_func.qmd")).unwrap();
    assert!(main_content.contains("[`helper_func`](helper_func.qmd)"));
}

#[test]
fn test_topic_index_from_ast_json_format() {
    let dir = tempdir().unwrap();
    let json_dir = tempdir().unwrap();

    let rd_roxygen = r#"% Generated by roxygen2: do not edit by hand
% Please edit documentation in R/coord-map.R
\name{coord_map}
\alias{coord_map}
\title{Map projections}
\description{Projects coordinates onto a map.}
"#;
    fs::write(dir.path().join("coord_map.Rd"), rd_roxygen).unwrap();

    export_package_ast(dir.path(), false, json_dir.path(), Some(1)).unwrap();

    let package =
        RdPackage::from_directory_with_format(json_dir.path(), false, InputFormat::AstJson)
            .unwrap();
    let options = TopicIndexOptions {
        output_extension: "qmd".to_string(),
        include_internal: false,
    };
    let index = generate_topic_index(&package, &options).unwrap();

    assert_eq!(index.topics.len(), 1);
    assert_eq!(index.topics[0].name, "coord_map");
    assert_eq!(
        index.topics[0].metadata.source_files,
        vec!["R/coord-map.R".to_string()]
    );
}

#[test]
fn test_ast_json_version_mismatch_reported_as_parse_error() {
    let json_dir = tempdir().unwrap();
    fs::write(
        json_dir.path().join("broken.json"),
        r#"{"version":99,"source":null,"sourceFiles":[],"document":{"sections":[]}}"#,
    )
    .unwrap();

    let result =
        RdPackage::from_directory_with_format(json_dir.path(), false, InputFormat::AstJson);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("broken.json"));
}
