//! Shared parsing facade for raw R documentation source.
//!
//! This crate owns rd2qmd's diagnostic policy for documents produced by
//! [`rd_source`]. It deliberately does not attach file names or other source
//! context; callers remain responsible for that presentation layer.

use std::fmt;

use rd_ast::RdDocument;

pub use rd_source::{
    Diagnostic, DiagnosticCode, ParseError as SourceParseError, Severity, SourcePosition,
    SourceSpan,
};

/// A parsed Rd document and its non-failing diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRd {
    document: RdDocument,
    diagnostics: Vec<Diagnostic>,
}

impl ParsedRd {
    /// Borrow the parsed document.
    pub fn document(&self) -> &RdDocument {
        &self.document
    }

    /// Borrow diagnostics emitted while parsing.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Consume this result into its document and diagnostics.
    pub fn into_parts(self) -> (RdDocument, Vec<Diagnostic>) {
        (self.document, self.diagnostics)
    }
}

/// A failure to produce an acceptable Rd document.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ParseFailure {
    /// The source parser could not produce a document.
    Source(SourceParseError),
    /// The source parser recovered a document but reported error diagnostics.
    Diagnostics(Vec<Diagnostic>),
}

impl ParseFailure {
    /// Return diagnostics associated with this failure, if any.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Source(_) => &[],
            Self::Diagnostics(diagnostics) => diagnostics,
        }
    }
}

impl fmt::Display for ParseFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Diagnostics(diagnostics) => {
                write!(
                    f,
                    "source parser reported {} error diagnostic(s)",
                    diagnostics
                        .iter()
                        .filter(|diagnostic| diagnostic.severity() == &Severity::Error)
                        .count()
                )?;
                if let Some(first) = diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.severity() == &Severity::Error)
                {
                    write!(f, ": {}", first.message())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ParseFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Diagnostics(_) => None,
        }
    }
}

impl From<SourceParseError> for ParseFailure {
    fn from(error: SourceParseError) -> Self {
        Self::Source(error)
    }
}

/// Parse raw Rd text according to rd2qmd's diagnostic policy.
///
/// Hard parser failures and error-severity diagnostics return [`ParseFailure`].
/// Warning diagnostics remain available alongside the successfully parsed
/// document.
pub fn parse(content: &str) -> Result<ParsedRd, ParseFailure> {
    let (document, diagnostics) = rd_source::parse(content.as_bytes())?.into_parts();

    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity() == &Severity::Error)
    {
        return Err(ParseFailure::Diagnostics(diagnostics));
    }

    Ok(ParsedRd {
        document,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticCode, ParseFailure, Severity, SourceParseError, parse};
    use rd_ast::{RdNode, RdTag};

    #[test]
    fn valid_source_returns_document_without_diagnostics() {
        let parsed = parse("\\name{example}\n\\title{An example}\n").unwrap();

        assert!(!parsed.document().nodes().is_empty());
        assert!(parsed.diagnostics().is_empty());
    }

    #[test]
    fn hard_parse_error_returns_failure() {
        let error = parse("\\name{before}\0\\name{after}").unwrap_err();

        assert_eq!(
            error,
            ParseFailure::Source(SourceParseError::NulByte { offset: 13 })
        );
    }

    #[test]
    fn recoverable_warning_returns_document_and_diagnostic() {
        let parsed = parse("\\examples{\n#ifdef unix\n}\nx <- 1\n#endif\ny <- 2\n}").unwrap();

        assert!(!parsed.document().nodes().is_empty());
        assert_eq!(parsed.diagnostics().len(), 1);
        assert_eq!(parsed.diagnostics()[0].severity(), &Severity::Warning);
        assert_eq!(
            parsed.diagnostics()[0].code(),
            &DiagnosticCode::UnexpectedClosingDelimiter
        );
    }

    #[test]
    fn error_diagnostic_returns_failure_with_all_diagnostics() {
        let error = parse("}").unwrap_err();

        let diagnostics = error.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity(), &Severity::Error);
        assert_eq!(
            diagnostics[0].code(),
            &DiagnosticCode::UnexpectedClosingDelimiter
        );
    }

    #[test]
    fn unknown_tag_wrapper_preserves_nested_paragraph_shape() {
        let parsed = rd_source::parse(b"\\madeUpTag{first\n\nsecond}").unwrap();
        assert_eq!(
            parsed.document().nodes(),
            &[RdNode::tagged(
                RdTag::Unknown(r"\madeUpTag".into()),
                None,
                vec![
                    RdNode::Text("first\n".into()),
                    RdNode::Text("\n".into()),
                    RdNode::Text("second".into()),
                ],
            )]
        );
        assert_eq!(parsed.diagnostics().len(), 1);
        assert_eq!(parsed.diagnostics()[0].severity(), &Severity::Error);
        assert_eq!(parsed.diagnostics()[0].code(), &DiagnosticCode::UnknownTag);
    }

    #[test]
    fn unknown_tag_wrapper_preserves_nested_itemize_shape() {
        let parsed = rd_source::parse(b"\\madeUpTag{\\itemize{\\item a}}").unwrap();
        assert_eq!(
            parsed.document().nodes(),
            &[RdNode::tagged(
                RdTag::Unknown(r"\madeUpTag".into()),
                None,
                vec![RdNode::tagged(
                    RdTag::Itemize,
                    None,
                    vec![
                        RdNode::tagged(RdTag::Item, None, vec![]),
                        RdNode::Text(" a".into()),
                    ],
                )],
            )]
        );
    }

    #[test]
    fn existing_fixtures_parse_without_diagnostics() {
        for name in ["basic.Rd", "sections.Rd", "markdown_codeblock.Rd"] {
            let content = std::fs::read_to_string(format!(
                "{}/../rd-parser/tests/fixtures/{name}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap();
            let parsed = parse(&content).unwrap();
            assert!(parsed.diagnostics().is_empty(), "fixture {name}");
        }
    }
}
