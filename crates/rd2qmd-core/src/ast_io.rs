//! JSON envelope for exchanging parsed Rd ASTs between tools
//!
//! The envelope wraps a [`RdDocument`] together with the metadata needed to
//! convert it later without re-reading the original `.Rd` file: the source
//! file name and the roxygen2-derived `source_files` list (the latter is not
//! part of the AST itself, since it comes from raw Rd header comments rather
//! than parsed sections).

use rd_ast::RdDocument;
use serde::{Deserialize, Serialize};

/// Version of the AST JSON envelope format
///
/// Bump this when the envelope shape or [`RdDocument`]'s serialized form
/// changes in a way that breaks older readers.
pub const AST_FORMAT_VERSION: u32 = 2;

/// Error type for AST envelope JSON I/O
#[derive(Debug, thiserror::Error)]
pub enum AstIoError {
    /// The JSON could not be parsed or does not match the envelope shape
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// The envelope's `version` field does not match [`AST_FORMAT_VERSION`]
    #[error("unsupported AST format version: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u64 },
}

/// A parsed Rd document paired with the metadata needed to convert it later
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RdAstEnvelope {
    /// AST format version, see [`AST_FORMAT_VERSION`]
    pub version: u32,
    /// Original `.Rd` file name (e.g. `"alpha.Rd"`)
    pub source: Option<String>,
    /// R source files that generated this Rd file (from roxygen2 comments)
    pub source_files: Vec<String>,
    /// The parsed Rd document
    pub document: RdDocument,
}

impl RdAstEnvelope {
    /// Create a new envelope at the current [`AST_FORMAT_VERSION`]
    pub fn new(document: RdDocument, source: Option<String>, source_files: Vec<String>) -> Self {
        Self {
            version: AST_FORMAT_VERSION,
            source,
            source_files,
            document,
        }
    }

    /// Serialize the envelope to a pretty-printed JSON string
    pub fn to_json_pretty(&self) -> Result<String, AstIoError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize an envelope from a JSON string
    ///
    /// Returns [`AstIoError::VersionMismatch`] if the envelope's `version`
    /// field does not match [`AST_FORMAT_VERSION`].
    pub fn from_json(json: &str) -> Result<Self, AstIoError> {
        let value: serde_json::Value = serde_json::from_str(json)?;

        if let Some(found) = value.get("version").and_then(|v| v.as_u64())
            && found != u64::from(AST_FORMAT_VERSION)
        {
            return Err(AstIoError::VersionMismatch {
                expected: AST_FORMAT_VERSION,
                found,
            });
        }

        Ok(serde_json::from_value(value)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rd_ast::{RdNode, RdTag};

    fn sample_document() -> RdDocument {
        RdDocument::new(vec![RdNode::tagged(
            RdTag::Name,
            None,
            vec![RdNode::Text("foo".to_string())],
        )])
    }

    #[test]
    fn test_envelope_roundtrip() {
        let envelope = RdAstEnvelope::new(
            sample_document(),
            Some("foo.Rd".to_string()),
            vec!["R/foo.R".to_string()],
        );

        let json = envelope.to_json_pretty().unwrap();
        let restored = RdAstEnvelope::from_json(&json).unwrap();

        assert_eq!(envelope, restored);
    }

    #[test]
    fn test_envelope_json_is_camel_case() {
        let envelope = RdAstEnvelope::new(
            sample_document(),
            Some("foo.Rd".to_string()),
            vec!["R/foo.R".to_string()],
        );

        let json = envelope.to_json_pretty().unwrap();
        assert!(json.contains("\"sourceFiles\""));
        assert!(!json.contains("\"source_files\""));
    }

    #[test]
    fn test_envelope_version_mismatch() {
        let json = r#"{"version":99,"source":null,"sourceFiles":[],"document":{"nodes":[]}}"#;
        let err = RdAstEnvelope::from_json(json).unwrap_err();
        assert!(matches!(
            err,
            AstIoError::VersionMismatch {
                expected: AST_FORMAT_VERSION,
                found: 99
            }
        ));
    }

    #[test]
    fn test_envelope_version_mismatch_beyond_u32() {
        let json =
            r#"{"version":4294967297,"source":null,"sourceFiles":[],"document":{"nodes":[]}}"#;
        let err = RdAstEnvelope::from_json(json).unwrap_err();
        assert!(matches!(
            err,
            AstIoError::VersionMismatch {
                expected: AST_FORMAT_VERSION,
                found: 4294967297
            }
        ));
    }
}
