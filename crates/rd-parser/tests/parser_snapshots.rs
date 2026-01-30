//! Snapshot tests for the Rd parser
//!
//! These tests parse Rd fixture files and snapshot the resulting AST
//! to detect unintended changes in parser behavior.

use std::fs;
use std::path::PathBuf;

use rd_parser::parse;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn parse_fixture(name: &str) -> String {
    let path = fixtures_dir().join(format!("{}.Rd", name));
    let source = fs::read_to_string(&path).expect("Failed to read fixture file");
    let doc = parse(&source).expect("Failed to parse fixture");
    serde_json::to_string_pretty(&doc).expect("Failed to serialize AST")
}

macro_rules! snapshot_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            let ast_json = parse_fixture(stringify!($name));
            insta::assert_snapshot!(ast_json);
        }
    };
}

snapshot_test!(basic);
snapshot_test!(formatting);
snapshot_test!(links);
snapshot_test!(lists);
snapshot_test!(tables);
snapshot_test!(equations);
snapshot_test!(conditionals);
snapshot_test!(methods);
snapshot_test!(examples);
snapshot_test!(sections);
snapshot_test!(nested);
snapshot_test!(figures);
snapshot_test!(lifecycle);
snapshot_test!(special_sections);
snapshot_test!(escapes);
snapshot_test!(preformatted);
snapshot_test!(markdown_codeblock);
snapshot_test!(new_tags);
