//! Roxygen2 markdown code block detection and conversion
//!
//! This module detects and converts roxygen2's markdown code block pattern
//! to properly formatted fenced code blocks with language specifiers.
//!
//! ## Pattern
//!
//! Roxygen2's markdown support converts fenced code blocks like:
//!
//! ````markdown
//! ```r
//! x <- 1 + 2
//! ```
//! ````
//!
//! Into the following Rd pattern:
//!
//! ```text
//! \if{html}{\out{<div class="sourceCode r">}}\preformatted{x <- 1 + 2
//! }\if{html}{\out{</div>}}
//! ```
//!
//! This module detects this pattern and extracts:
//! - The language from the `sourceCode [lang]` class
//! - The code content from `\preformatted{}`
//!
//! ## Supported Patterns
//!
//! | Class | Language |
//! |-------|----------|
//! | `sourceCode r` | `r` |
//! | `sourceCode python` | `python` |
//! | `sourceCode yaml` | `yaml` |
//! | `sourceCode` (no lang) | None |
//! | `r` (R6 method usage) | `r` |

use rd_parser::RdNode;

/// Result of matching a roxygen2 markdown code block pattern
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoxygenCodeBlock {
    /// The language extracted from the sourceCode class (e.g., "r", "python")
    /// None if no language was specified
    pub language: Option<String>,
    /// The code content
    pub code: String,
    /// Number of nodes consumed (typically 3: opening if, preformatted, closing if)
    pub nodes_consumed: usize,
}

/// Try to match the roxygen2 markdown code block pattern at the current position
///
/// Pattern:
/// 1. `RdNode::If { format: "html", content: [RdNode::Out("<div class=\"sourceCode [lang]\">")] }`
/// 2. `RdNode::Preformatted(code)`
/// 3. `RdNode::If { format: "html", content: [RdNode::Out("</div>")] }`
///
/// Returns `Some(RoxygenCodeBlock)` if pattern matches, `None` otherwise.
pub fn try_match_roxygen_code_block(nodes: &[RdNode]) -> Option<RoxygenCodeBlock> {
    if nodes.len() < 3 {
        return None;
    }

    // Match first node: \if{html}{\out{<div class="sourceCode ...">}}
    let language = match_opening_div(&nodes[0])?;

    // Match second node: \preformatted{...}
    let code = match &nodes[1] {
        RdNode::Preformatted(code) => code.clone(),
        _ => return None,
    };

    // Match third node: \if{html}{\out{</div>}}
    if !match_closing_div(&nodes[2]) {
        return None;
    }

    Some(RoxygenCodeBlock {
        language,
        code,
        nodes_consumed: 3,
    })
}

/// Match the opening div pattern and extract the language
///
/// Matches: `\if{html}{\out{<div class="sourceCode [lang]">}}`
/// or: `\if{html}{\out{<div class="r">}}` (R6 method usage)
///
/// Returns:
/// - `Some(Some(lang))` if language is specified
/// - `Some(None)` if no language (just "sourceCode")
/// - `None` if pattern doesn't match
fn match_opening_div(node: &RdNode) -> Option<Option<String>> {
    let RdNode::If { format, content } = node else {
        return None;
    };

    if format != "html" {
        return None;
    }

    // Expect single Out node
    if content.len() != 1 {
        return None;
    }

    let RdNode::Out(html) = &content[0] else {
        return None;
    };

    // Parse the HTML div tag to extract language
    extract_language_from_div(html)
}

/// Extract language from a div class attribute
///
/// Handles:
/// - `<div class="sourceCode r">` -> Some(Some("r"))
/// - `<div class="sourceCode python">` -> Some(Some("python"))
/// - `<div class="sourceCode">` -> Some(None)
/// - `<div class="r">` -> Some(Some("r")) (R6 usage pattern)
fn extract_language_from_div(html: &str) -> Option<Option<String>> {
    // Check for sourceCode pattern
    if let Some(class_start) = html.find("class=\"") {
        let after_class = &html[class_start + 7..];
        if let Some(class_end) = after_class.find('"') {
            let class_value = &after_class[..class_end];

            // Check for "sourceCode [lang]" pattern
            if let Some(rest) = class_value.strip_prefix("sourceCode") {
                let rest = rest.trim();
                if rest.is_empty() {
                    // Just "sourceCode" with no language
                    return Some(None);
                } else {
                    // "sourceCode r" or "sourceCode python" etc.
                    return Some(Some(rest.to_string()));
                }
            }

            // Check for "r" class (R6 method usage)
            if class_value == "r" {
                return Some(Some("r".to_string()));
            }
        }
    }

    None
}

/// Match the closing div pattern
///
/// Matches: `\if{html}{\out{</div>}}`
fn match_closing_div(node: &RdNode) -> bool {
    let RdNode::If { format, content } = node else {
        return false;
    };

    if format != "html" {
        return false;
    }

    if content.len() != 1 {
        return false;
    }

    let RdNode::Out(html) = &content[0] else {
        return false;
    };

    html.trim() == "</div>"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_opening_if(class: &str) -> RdNode {
        RdNode::If {
            format: "html".to_string(),
            content: vec![RdNode::Out(format!("<div class=\"{}\">", class))],
        }
    }

    fn make_closing_if() -> RdNode {
        RdNode::If {
            format: "html".to_string(),
            content: vec![RdNode::Out("</div>".to_string())],
        }
    }

    fn make_preformatted(code: &str) -> RdNode {
        RdNode::Preformatted(code.to_string())
    }

    #[test]
    fn test_match_r_code_block() {
        let nodes = vec![
            make_opening_if("sourceCode r"),
            make_preformatted("x <- 1 + 2\ny <- x * 3"),
            make_closing_if(),
        ];

        let result = try_match_roxygen_code_block(&nodes);
        assert!(result.is_some());

        let block = result.unwrap();
        assert_eq!(block.language, Some("r".to_string()));
        assert_eq!(block.code, "x <- 1 + 2\ny <- x * 3");
        assert_eq!(block.nodes_consumed, 3);
    }

    #[test]
    fn test_match_python_code_block() {
        let nodes = vec![
            make_opening_if("sourceCode python"),
            make_preformatted("def hello():\n    print('hello')"),
            make_closing_if(),
        ];

        let result = try_match_roxygen_code_block(&nodes);
        assert!(result.is_some());

        let block = result.unwrap();
        assert_eq!(block.language, Some("python".to_string()));
        assert_eq!(block.code, "def hello():\n    print('hello')");
    }

    #[test]
    fn test_match_no_language() {
        let nodes = vec![
            make_opening_if("sourceCode"),
            make_preformatted("plain text"),
            make_closing_if(),
        ];

        let result = try_match_roxygen_code_block(&nodes);
        assert!(result.is_some());

        let block = result.unwrap();
        assert_eq!(block.language, None);
        assert_eq!(block.code, "plain text");
    }

    #[test]
    fn test_match_r6_usage_pattern() {
        // R6 method usage uses just class="r" without "sourceCode"
        let nodes = vec![
            make_opening_if("r"),
            make_preformatted("hello_r6$new()"),
            make_closing_if(),
        ];

        let result = try_match_roxygen_code_block(&nodes);
        assert!(result.is_some());

        let block = result.unwrap();
        assert_eq!(block.language, Some("r".to_string()));
    }

    #[test]
    fn test_no_match_wrong_format() {
        let nodes = vec![
            RdNode::If {
                format: "latex".to_string(), // Not html
                content: vec![RdNode::Out("<div class=\"sourceCode r\">".to_string())],
            },
            make_preformatted("code"),
            make_closing_if(),
        ];

        assert!(try_match_roxygen_code_block(&nodes).is_none());
    }

    #[test]
    fn test_no_match_standalone_preformatted() {
        // Standalone preformatted without the if/out wrapper
        let nodes = vec![make_preformatted("code")];

        assert!(try_match_roxygen_code_block(&nodes).is_none());
    }

    #[test]
    fn test_no_match_too_few_nodes() {
        let nodes = vec![
            make_opening_if("sourceCode r"),
            make_preformatted("code"),
            // Missing closing if
        ];

        assert!(try_match_roxygen_code_block(&nodes).is_none());
    }

    #[test]
    fn test_no_match_wrong_class() {
        let nodes = vec![
            make_opening_if("someOtherClass"),
            make_preformatted("code"),
            make_closing_if(),
        ];

        assert!(try_match_roxygen_code_block(&nodes).is_none());
    }

    #[test]
    fn test_extract_language_yaml() {
        let nodes = vec![
            make_opening_if("sourceCode yaml"),
            make_preformatted("key: value"),
            make_closing_if(),
        ];

        let result = try_match_roxygen_code_block(&nodes);
        assert!(result.is_some());
        assert_eq!(result.unwrap().language, Some("yaml".to_string()));
    }

    #[test]
    fn test_extract_language_sql() {
        let nodes = vec![
            make_opening_if("sourceCode sql"),
            make_preformatted("SELECT * FROM table"),
            make_closing_if(),
        ];

        let result = try_match_roxygen_code_block(&nodes);
        assert!(result.is_some());
        assert_eq!(result.unwrap().language, Some("sql".to_string()));
    }
}
