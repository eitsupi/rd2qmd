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

impl RdDocument {
    /// Extract lifecycle stage from this Rd document.
    ///
    /// Searches the description section for lifecycle badges in the format:
    /// `\figure{lifecycle-<stage>.svg}{...}`
    ///
    /// Returns the stage name (e.g., "deprecated", "experimental", "superseded", "stable")
    /// or None if no lifecycle badge is found.
    ///
    /// # Example
    ///
    /// ```
    /// use rd_parser::parse;
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
    /// assert_eq!(doc.lifecycle(), Some("deprecated".to_string()));
    /// ```
    pub fn lifecycle(&self) -> Option<String> {
        // Find the description section
        let description = self.get_section(&SectionTag::Description)?;

        // Search for lifecycle figure in the description content
        find_lifecycle_in_nodes(&description.content)
    }
}

/// Recursively search for a lifecycle figure in a slice of nodes.
fn find_lifecycle_in_nodes(nodes: &[RdNode]) -> Option<String> {
    for node in nodes {
        if let Some(lifecycle) = find_lifecycle_in_node(node) {
            return Some(lifecycle);
        }
    }
    None
}

/// Search for a lifecycle figure in a single node and its children.
fn find_lifecycle_in_node(node: &RdNode) -> Option<String> {
    match node {
        // Check Figure nodes directly
        RdNode::Figure { file, .. } => extract_lifecycle_from_filename(file),

        // Recursively search nodes that can contain Figure
        RdNode::IfElse {
            then_content,
            else_content,
            ..
        } => find_lifecycle_in_nodes(then_content)
            .or_else(|| find_lifecycle_in_nodes(else_content)),

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
        | RdNode::DontShow(children) => find_lifecycle_in_nodes(children),

        RdNode::Itemize(items) | RdNode::Enumerate(items) => find_lifecycle_in_nodes(items),

        RdNode::Item { content, label } => {
            if let Some(label_nodes) = label {
                if let Some(lifecycle) = find_lifecycle_in_nodes(label_nodes) {
                    return Some(lifecycle);
                }
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
        | RdNode::Acronym(_)
        | RdNode::Option(_)
        | RdNode::Var(_)
        | RdNode::Env(_)
        | RdNode::Command(_) => None,
    }
}

/// Extract lifecycle stage from a figure filename.
///
/// Looks for filenames matching the pattern `lifecycle-<stage>.svg` or similar.
fn extract_lifecycle_from_filename(filename: &str) -> Option<String> {
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
    let stage = basename
        .strip_prefix("lifecycle-")
        .and_then(|s| s.rsplit('.').last())
        .or_else(|| {
            // Handle case where there's no extension
            basename.strip_prefix("lifecycle-")
        })?;

    if stage.is_empty() {
        return None;
    }

    Some(stage.to_string())
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
        assert_eq!(doc.lifecycle(), Some("deprecated".to_string()));
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
        assert_eq!(doc.lifecycle(), Some("experimental".to_string()));
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
        assert_eq!(doc.lifecycle(), Some("stable".to_string()));
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
        assert_eq!(doc.lifecycle(), Some("superseded".to_string()));
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
        assert_eq!(doc.lifecycle(), Some("questioning".to_string()));
    }

    #[test]
    fn test_extract_lifecycle_from_filename() {
        assert_eq!(
            extract_lifecycle_from_filename("lifecycle-deprecated.svg"),
            Some("deprecated".to_string())
        );
        assert_eq!(
            extract_lifecycle_from_filename("figures/lifecycle-experimental.png"),
            Some("experimental".to_string())
        );
        assert_eq!(
            extract_lifecycle_from_filename("man/figures/lifecycle-stable.svg"),
            Some("stable".to_string())
        );
        assert_eq!(extract_lifecycle_from_filename("some-other.svg"), None);
        assert_eq!(extract_lifecycle_from_filename("deprecated.svg"), None);
    }
}
