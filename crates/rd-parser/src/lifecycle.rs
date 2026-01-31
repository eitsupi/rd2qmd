//! Lifecycle badge extraction for Rd documents
//!
//! This module provides functionality to extract lifecycle status from Rd documents.
//! It detects lifecycle badges embedded by the lifecycle R package in the format:
//!
//! ```rd
//! \ifelse{html}{\href{URL}{\figure{lifecycle-deprecated.svg}{...}}}{\strong{[Deprecated]}}
//! ```
//!
//! This is a feature-gated module (`lifecycle` feature) because lifecycle badges
//! are not part of the standard Rd specification but a convention established by
//! the lifecycle R package and pkgdown.

use crate::ast::{RdDocument, RdNode, SectionTag};
use std::fmt;
use std::str::FromStr;

/// Lifecycle stage for R functions and packages.
///
/// See <https://lifecycle.r-lib.org/articles/stages.html> for the official documentation.
///
/// # Current Stages
///
/// The lifecycle package defines four official stages:
///
/// | Stage | Description |
/// |-------|-------------|
/// | [`Experimental`](Lifecycle::Experimental) | No promises for long-term stability |
/// | [`Stable`](Lifecycle::Stable) | Default stage, function works as expected |
/// | [`Superseded`](Lifecycle::Superseded) | Better alternative exists, but won't be removed |
/// | [`Deprecated`](Lifecycle::Deprecated) | Scheduled for removal, emits warnings |
///
/// # Legacy Stages (for backwards compatibility only)
///
/// These stages are no longer recommended by the lifecycle package but are still
/// recognized for backwards compatibility with older packages:
///
/// | Stage | Description |
/// |-------|-------------|
/// | [`Maturing`](Lifecycle::Maturing) | Previously used between experimental and stable |
/// | [`Questioning`](Lifecycle::Questioning) | Author has doubts about the function |
/// | [`SoftDeprecated`](Lifecycle::SoftDeprecated) | Gentler form before full deprecation |
/// | [`Defunct`](Lifecycle::Defunct) | Function exists but always errors |
/// | [`Retired`](Lifecycle::Retired) | Old name for superseded |
///
/// Use [`is_current()`](Lifecycle::is_current) and [`is_legacy()`](Lifecycle::is_legacy)
/// to check which category a stage belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "json", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum Lifecycle {
    // ---- Current stages (recommended) ----
    /// Experimental: no promises for long-term stability.
    Experimental,
    /// Stable: default stage, function works as expected.
    Stable,
    /// Superseded: better alternative exists, but won't be removed.
    Superseded,
    /// Deprecated: scheduled for removal, emits warnings.
    Deprecated,

    // ---- Legacy stages (for backwards compatibility) ----
    /// **Legacy.** Maturing: previously used between experimental and stable.
    Maturing,
    /// **Legacy.** Questioning: author has doubts about the function.
    Questioning,
    /// **Legacy.** Soft-deprecated: gentler form before full deprecation.
    SoftDeprecated,
    /// **Legacy.** Defunct: function exists but always errors.
    Defunct,
    /// **Legacy.** Retired: old name for superseded.
    Retired,
}

impl Lifecycle {
    /// Returns true if this is a current (recommended) lifecycle stage.
    pub fn is_current(&self) -> bool {
        matches!(
            self,
            Lifecycle::Experimental
                | Lifecycle::Stable
                | Lifecycle::Superseded
                | Lifecycle::Deprecated
        )
    }

    /// Returns true if this is a legacy (deprecated) lifecycle stage.
    pub fn is_legacy(&self) -> bool {
        !self.is_current()
    }

    /// Returns the canonical stage name as used in badge filenames.
    ///
    /// For example, `Lifecycle::SoftDeprecated` returns `"soft-deprecated"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Lifecycle::Experimental => "experimental",
            Lifecycle::Stable => "stable",
            Lifecycle::Superseded => "superseded",
            Lifecycle::Deprecated => "deprecated",
            Lifecycle::Maturing => "maturing",
            Lifecycle::Questioning => "questioning",
            Lifecycle::SoftDeprecated => "soft-deprecated",
            Lifecycle::Defunct => "defunct",
            Lifecycle::Retired => "retired",
        }
    }
}

impl fmt::Display for Lifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Error returned when parsing an invalid lifecycle stage string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseLifecycleError {
    /// The invalid input string
    pub input: String,
}

impl fmt::Display for ParseLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown lifecycle stage: '{}'. Expected one of: experimental, stable, superseded, deprecated, maturing, questioning, soft-deprecated, defunct, retired",
            self.input
        )
    }
}

impl std::error::Error for ParseLifecycleError {}

impl FromStr for Lifecycle {
    type Err = ParseLifecycleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "experimental" => Ok(Lifecycle::Experimental),
            "stable" => Ok(Lifecycle::Stable),
            "superseded" => Ok(Lifecycle::Superseded),
            "deprecated" => Ok(Lifecycle::Deprecated),
            "maturing" => Ok(Lifecycle::Maturing),
            "questioning" => Ok(Lifecycle::Questioning),
            "soft-deprecated" => Ok(Lifecycle::SoftDeprecated),
            "defunct" => Ok(Lifecycle::Defunct),
            "retired" => Ok(Lifecycle::Retired),
            _ => Err(ParseLifecycleError {
                input: s.to_string(),
            }),
        }
    }
}

impl RdDocument {
    /// Extract lifecycle stage from this Rd document.
    ///
    /// Searches the description section for lifecycle badges in the format:
    /// `\figure{lifecycle-<stage>.svg}{...}`
    ///
    /// Returns the [`Lifecycle`] stage or `None` if no lifecycle badge is found.
    ///
    /// # Example
    ///
    /// ```
    /// use rd_parser::{parse, Lifecycle};
    ///
    /// let source = r#"
    /// \name{example}
    /// \description{
    /// \ifelse{html}{\href{https://lifecycle.r-lib.org/}{\figure{lifecycle-deprecated.svg}{options: alt='[Deprecated]'}}}{\strong{[Deprecated]}}
    /// A deprecated function.
    /// }
    /// "#;
    ///
    /// let doc = parse(source).unwrap();
    /// assert_eq!(doc.lifecycle(), Some(Lifecycle::Deprecated));
    /// ```
    pub fn lifecycle(&self) -> Option<Lifecycle> {
        // Find the description section
        let description = self.get_section(&SectionTag::Description)?;

        // Search for lifecycle figure in the description content
        find_lifecycle_in_nodes(&description.content)
    }
}

/// Recursively search for a lifecycle figure in a slice of nodes.
fn find_lifecycle_in_nodes(nodes: &[RdNode]) -> Option<Lifecycle> {
    for node in nodes {
        if let Some(lifecycle) = find_lifecycle_in_node(node) {
            return Some(lifecycle);
        }
    }
    None
}

/// Search for a lifecycle figure in a single node and its children.
fn find_lifecycle_in_node(node: &RdNode) -> Option<Lifecycle> {
    match node {
        // Check Figure nodes directly
        RdNode::Figure { file, .. } => extract_lifecycle_from_filename(file),

        // Recursively search nodes that can contain Figure
        RdNode::IfElse {
            then_content,
            else_content,
            ..
        } => {
            find_lifecycle_in_nodes(then_content).or_else(|| find_lifecycle_in_nodes(else_content))
        }

        RdNode::If { content, .. } => find_lifecycle_in_nodes(content),

        RdNode::Href { text, .. } => find_lifecycle_in_nodes(text),

        RdNode::Code(children)
        | RdNode::Emph(children)
        | RdNode::Strong(children)
        | RdNode::Paragraph(children)
        | RdNode::Samp(children)
        | RdNode::File(children)
        | RdNode::Dfn(children)
        | RdNode::Kbd(children)
        | RdNode::SQuote(children)
        | RdNode::DQuote(children)
        | RdNode::DontRun(children)
        | RdNode::DontTest(children)
        | RdNode::DontShow(children)
        | RdNode::DontDiff(children) => find_lifecycle_in_nodes(children),

        RdNode::Itemize(items) | RdNode::Enumerate(items) => find_lifecycle_in_nodes(items),

        RdNode::Item { content, label } => {
            if let Some(label_nodes) = label
                && let Some(lifecycle) = find_lifecycle_in_nodes(label_nodes)
            {
                return Some(lifecycle);
            }
            find_lifecycle_in_nodes(content)
        }

        RdNode::Section { content, title } | RdNode::Subsection { content, title } => {
            find_lifecycle_in_nodes(title).or_else(|| find_lifecycle_in_nodes(content))
        }

        RdNode::Describe(items) => {
            for item in items {
                if let Some(lifecycle) = find_lifecycle_in_nodes(&item.term) {
                    return Some(lifecycle);
                }
                if let Some(lifecycle) = find_lifecycle_in_nodes(&item.description) {
                    return Some(lifecycle);
                }
            }
            None
        }

        RdNode::Tabular { rows, .. } => {
            for row in rows {
                for cell in row {
                    if let Some(lifecycle) = find_lifecycle_in_nodes(cell) {
                        return Some(lifecycle);
                    }
                }
            }
            None
        }

        RdNode::Macro { args, .. } => {
            for arg in args {
                if let Some(lifecycle) = find_lifecycle_in_nodes(arg) {
                    return Some(lifecycle);
                }
            }
            None
        }

        RdNode::Link { text, .. } => {
            if let Some(text_nodes) = text {
                find_lifecycle_in_nodes(text_nodes)
            } else {
                None
            }
        }

        // Terminal nodes - no children to search
        RdNode::Text(_)
        | RdNode::Verbatim(_)
        | RdNode::Verb(_)
        | RdNode::Preformatted(_)
        | RdNode::Url(_)
        | RdNode::Email(_)
        | RdNode::Pkg(_)
        | RdNode::Eqn { .. }
        | RdNode::Deqn { .. }
        | RdNode::Sexpr { .. }
        | RdNode::Special(_)
        | RdNode::LineBreak
        | RdNode::Tab
        | RdNode::Out(_)
        | RdNode::Method { .. }
        | RdNode::S4Method { .. }
        | RdNode::S3Method { .. }
        | RdNode::Acronym(_)
        | RdNode::Abbr(_)
        | RdNode::Cite(_)
        | RdNode::Option(_)
        | RdNode::Var(_)
        | RdNode::Env(_)
        | RdNode::Command(_)
        | RdNode::Doi(_)
        | RdNode::LinkS4Class { .. }
        | RdNode::Enc { .. } => None,
    }
}

/// Extract lifecycle stage from a figure filename.
///
/// Looks for filenames matching the pattern `lifecycle-<stage>.svg` or similar.
fn extract_lifecycle_from_filename(filename: &str) -> Option<Lifecycle> {
    // Check if the filename contains "lifecycle-"
    if !filename.contains("lifecycle-") {
        return None;
    }

    // Extract the stage name from patterns like:
    // - "lifecycle-deprecated.svg"
    // - "figures/lifecycle-experimental.png"
    // - "man/figures/lifecycle-stable.svg"
    let basename = filename.rsplit('/').next().unwrap_or(filename);

    // Remove "lifecycle-" prefix and file extension
    let stage_str = basename.strip_prefix("lifecycle-").and_then(|s| {
        // Split off the file extension if present
        s.rsplit_once('.')
            .map(|(name, _ext)| name)
            .or(Some(s)) // No extension case
    })?;

    if stage_str.is_empty() {
        return None;
    }

    // Parse the stage string into a Lifecycle enum
    stage_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn test_lifecycle_deprecated() {
        let source = r#"
\name{example}
\description{
\ifelse{html}{\href{https://lifecycle.r-lib.org/articles/stages.html#deprecated}{\figure{lifecycle-deprecated.svg}{options: alt='[Deprecated]'}}}{\strong{[Deprecated]}}

A deprecated function.
}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), Some(Lifecycle::Deprecated));
    }

    #[test]
    fn test_lifecycle_experimental() {
        let source = r#"
\name{example}
\description{
\ifelse{html}{\href{https://lifecycle.r-lib.org/}{\figure{lifecycle-experimental.svg}{alt='[Experimental]'}}}{\strong{[Experimental]}}
}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), Some(Lifecycle::Experimental));
    }

    #[test]
    fn test_lifecycle_stable() {
        let source = r#"
\name{example}
\description{
\ifelse{html}{\figure{lifecycle-stable.svg}}{\strong{[Stable]}}
}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), Some(Lifecycle::Stable));
    }

    #[test]
    fn test_lifecycle_superseded() {
        let source = r#"
\name{example}
\description{
\ifelse{html}{\href{https://lifecycle.r-lib.org/}{\figure{lifecycle-superseded.svg}{}}}{\strong{[Superseded]}}
}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), Some(Lifecycle::Superseded));
    }

    #[test]
    fn test_no_lifecycle() {
        let source = r#"
\name{example}
\description{A normal function without lifecycle badge.}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), None);
    }

    #[test]
    fn test_no_description() {
        let source = r#"
\name{example}
\title{Example}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), None);
    }

    #[test]
    fn test_figure_not_lifecycle() {
        let source = r#"
\name{example}
\description{
\figure{some-other-figure.png}
A function with a non-lifecycle figure.
}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), None);
    }

    #[test]
    fn test_lifecycle_in_path() {
        let source = r#"
\name{example}
\description{
\figure{man/figures/lifecycle-questioning.svg}{alt='[Questioning]'}
}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), Some(Lifecycle::Questioning));
    }

    #[test]
    fn test_extract_lifecycle_from_filename() {
        assert_eq!(
            extract_lifecycle_from_filename("lifecycle-deprecated.svg"),
            Some(Lifecycle::Deprecated)
        );
        assert_eq!(
            extract_lifecycle_from_filename("figures/lifecycle-experimental.png"),
            Some(Lifecycle::Experimental)
        );
        assert_eq!(
            extract_lifecycle_from_filename("man/figures/lifecycle-stable.svg"),
            Some(Lifecycle::Stable)
        );
        assert_eq!(extract_lifecycle_from_filename("some-other.svg"), None);
        assert_eq!(extract_lifecycle_from_filename("deprecated.svg"), None);
    }

    // Tests for Lifecycle enum
    #[test]
    fn test_lifecycle_from_str() {
        assert_eq!(
            "experimental".parse::<Lifecycle>(),
            Ok(Lifecycle::Experimental)
        );
        assert_eq!("stable".parse::<Lifecycle>(), Ok(Lifecycle::Stable));
        assert_eq!("superseded".parse::<Lifecycle>(), Ok(Lifecycle::Superseded));
        assert_eq!("deprecated".parse::<Lifecycle>(), Ok(Lifecycle::Deprecated));
        assert_eq!("maturing".parse::<Lifecycle>(), Ok(Lifecycle::Maturing));
        assert_eq!(
            "questioning".parse::<Lifecycle>(),
            Ok(Lifecycle::Questioning)
        );
        assert_eq!(
            "soft-deprecated".parse::<Lifecycle>(),
            Ok(Lifecycle::SoftDeprecated)
        );
        assert_eq!("defunct".parse::<Lifecycle>(), Ok(Lifecycle::Defunct));

        // "softdeprecated" (no hyphen) should be rejected
        assert!("softdeprecated".parse::<Lifecycle>().is_err());
        assert_eq!("retired".parse::<Lifecycle>(), Ok(Lifecycle::Retired));

        // Case insensitive
        assert_eq!(
            "EXPERIMENTAL".parse::<Lifecycle>(),
            Ok(Lifecycle::Experimental)
        );
        assert_eq!("Deprecated".parse::<Lifecycle>(), Ok(Lifecycle::Deprecated));

        // Invalid
        assert!("unknown".parse::<Lifecycle>().is_err());
    }

    #[test]
    fn test_lifecycle_display() {
        assert_eq!(Lifecycle::Experimental.to_string(), "experimental");
        assert_eq!(Lifecycle::Stable.to_string(), "stable");
        assert_eq!(Lifecycle::Superseded.to_string(), "superseded");
        assert_eq!(Lifecycle::Deprecated.to_string(), "deprecated");
        assert_eq!(Lifecycle::Maturing.to_string(), "maturing");
        assert_eq!(Lifecycle::Questioning.to_string(), "questioning");
        assert_eq!(Lifecycle::SoftDeprecated.to_string(), "soft-deprecated");
        assert_eq!(Lifecycle::Defunct.to_string(), "defunct");
        assert_eq!(Lifecycle::Retired.to_string(), "retired");
    }

    #[test]
    fn test_lifecycle_is_current() {
        assert!(Lifecycle::Experimental.is_current());
        assert!(Lifecycle::Stable.is_current());
        assert!(Lifecycle::Superseded.is_current());
        assert!(Lifecycle::Deprecated.is_current());

        assert!(!Lifecycle::Maturing.is_current());
        assert!(!Lifecycle::Questioning.is_current());
        assert!(!Lifecycle::SoftDeprecated.is_current());
        assert!(!Lifecycle::Defunct.is_current());
        assert!(!Lifecycle::Retired.is_current());
    }

    #[test]
    fn test_lifecycle_is_legacy() {
        assert!(!Lifecycle::Experimental.is_legacy());
        assert!(!Lifecycle::Stable.is_legacy());
        assert!(!Lifecycle::Superseded.is_legacy());
        assert!(!Lifecycle::Deprecated.is_legacy());

        assert!(Lifecycle::Maturing.is_legacy());
        assert!(Lifecycle::Questioning.is_legacy());
        assert!(Lifecycle::SoftDeprecated.is_legacy());
        assert!(Lifecycle::Defunct.is_legacy());
        assert!(Lifecycle::Retired.is_legacy());
    }

    #[test]
    fn test_lifecycle_legacy_stages() {
        // Test that legacy stages can be detected from Rd files
        let source = r#"
\name{example}
\description{
\figure{lifecycle-maturing.svg}
}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), Some(Lifecycle::Maturing));

        let source = r#"
\name{example}
\description{
\figure{lifecycle-soft-deprecated.svg}
}
"#;
        let doc = parse(source).unwrap();
        assert_eq!(doc.lifecycle(), Some(Lifecycle::SoftDeprecated));
    }

    #[cfg(feature = "json")]
    #[test]
    fn test_lifecycle_json_serialization() {
        // Test JSON serialization uses snake_case
        let json = serde_json::to_string(&Lifecycle::Experimental).unwrap();
        assert_eq!(json, r#""experimental""#);

        let json = serde_json::to_string(&Lifecycle::SoftDeprecated).unwrap();
        assert_eq!(json, r#""soft_deprecated""#);

        // Test deserialization
        let lifecycle: Lifecycle = serde_json::from_str(r#""deprecated""#).unwrap();
        assert_eq!(lifecycle, Lifecycle::Deprecated);

        let lifecycle: Lifecycle = serde_json::from_str(r#""soft_deprecated""#).unwrap();
        assert_eq!(lifecycle, Lifecycle::SoftDeprecated);
    }
}
