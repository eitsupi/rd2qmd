//! Internal parsing support for the `rd-source` migration.
//!
//! This module is intentionally not wired into the public conversion API yet;
//! that integration is part of a later migration phase.

use rd_ast::RdDocument;

#[cfg(test)]
pub(crate) fn parse_with_rd_source(
    content: &str,
) -> Result<rd_source::Parsed, rd_source::ParseError> {
    rd_source::parse(content.as_bytes())
}

/// Extract owned source paths from a recognized generation header.
pub(crate) fn extract_source_files(document: &RdDocument) -> Vec<String> {
    document
        .generation_header()
        .map(|header| {
            header
                .source_files()
                .iter()
                .map(|path| (*path).to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{extract_source_files, parse_with_rd_source};

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/../rd-parser/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn parses_existing_fixtures_without_diagnostics() {
        for name in ["basic.Rd", "sections.Rd", "markdown_codeblock.Rd"] {
            let parsed = parse_with_rd_source(&fixture(name)).unwrap();
            assert!(parsed.diagnostics().is_empty(), "fixture {name}");
        }
    }

    #[test]
    fn extracts_roxygen_source_files_as_owned_paths() {
        let parsed = parse_with_rd_source(&fixture("markdown_codeblock.Rd")).unwrap();
        let document = parsed.document();

        assert_eq!(
            extract_source_files(document),
            ["R/markdown_codeblock.R".to_owned()]
        );
    }

    #[test]
    fn returns_no_source_files_without_a_generation_header() {
        let parsed = parse_with_rd_source(&fixture("sections.Rd")).unwrap();

        assert!(extract_source_files(parsed.document()).is_empty());
    }
}
