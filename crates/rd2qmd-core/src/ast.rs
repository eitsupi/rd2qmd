//! Rd (R Documentation) AST types
//!
//! This module defines the abstract syntax tree for Rd files.
//! Reference: https://cran.r-project.org/doc/manuals/r-release/R-exts.html#Rd-format

use serde::{Deserialize, Serialize};

/// A complete Rd document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RdDocument {
    /// Top-level sections in the document
    pub sections: Vec<RdSection>,
}

/// A top-level section in an Rd document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RdSection {
    /// Section tag name (e.g., "name", "title", "description")
    pub tag: SectionTag,
    /// Section content
    pub content: Vec<RdNode>,
}

/// Known section tags in Rd format
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionTag {
    // Required sections
    Name,
    Title,
    Description,

    // Common sections
    Alias,
    Usage,
    Arguments,
    Value,
    Details,
    Note,
    Author,
    References,
    SeeAlso,
    Examples,
    Keyword,
    Concept,
    Format,
    Source,

    // Custom sections
    Section(String),

    // Encoding declaration
    Encoding,

    // Documentation type
    DocType,

    // R version requirement
    RdVersion,

    // Unknown section (for forward compatibility)
    Unknown(String),
}

impl SectionTag {
    /// Parse a section tag from a string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "name" => Self::Name,
            "title" => Self::Title,
            "description" => Self::Description,
            "alias" => Self::Alias,
            "usage" => Self::Usage,
            "arguments" => Self::Arguments,
            "value" => Self::Value,
            "details" => Self::Details,
            "note" => Self::Note,
            "author" => Self::Author,
            "references" => Self::References,
            "seealso" => Self::SeeAlso,
            "examples" => Self::Examples,
            "keyword" => Self::Keyword,
            "concept" => Self::Concept,
            "format" => Self::Format,
            "source" => Self::Source,
            "encoding" => Self::Encoding,
            "doctype" => Self::DocType,
            "rdversion" => Self::RdVersion,
            _ => Self::Unknown(s.to_string()),
        }
    }

    /// Get the tag name as a string
    pub fn as_str(&self) -> &str {
        match self {
            Self::Name => "name",
            Self::Title => "title",
            Self::Description => "description",
            Self::Alias => "alias",
            Self::Usage => "usage",
            Self::Arguments => "arguments",
            Self::Value => "value",
            Self::Details => "details",
            Self::Note => "note",
            Self::Author => "author",
            Self::References => "references",
            Self::SeeAlso => "seealso",
            Self::Examples => "examples",
            Self::Keyword => "keyword",
            Self::Concept => "concept",
            Self::Format => "format",
            Self::Source => "source",
            Self::Encoding => "encoding",
            Self::DocType => "doctype",
            Self::RdVersion => "rdversion",
            Self::Section(name) => name,
            Self::Unknown(name) => name,
        }
    }
}

/// A node in the Rd AST (can be block or inline)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RdNode {
    /// Plain text content
    Text(String),

    /// A paragraph (sequence of inline content separated by blank lines)
    Paragraph(Vec<RdNode>),

    /// Verbatim/preformatted text block
    Verbatim(String),

    /// Custom section with title
    Section { title: Vec<RdNode>, content: Vec<RdNode> },

    /// Subsection within a section
    Subsection { title: Vec<RdNode>, content: Vec<RdNode> },

    /// Itemized (bullet) list
    Itemize(Vec<RdNode>),

    /// Enumerated (numbered) list
    Enumerate(Vec<RdNode>),

    /// Description list (term-definition pairs)
    Describe(Vec<DescribeItem>),

    /// List item
    Item {
        /// Optional label (for description lists)
        label: Option<Vec<RdNode>>,
        /// Item content
        content: Vec<RdNode>,
    },

    /// Table
    Tabular {
        /// Column alignment specification (l, c, r)
        alignment: String,
        /// Table rows
        rows: Vec<Vec<Vec<RdNode>>>,
    },

    /// Inline code (\code{})
    Code(Vec<RdNode>),

    /// Verbatim inline (\verb{})
    Verb(String),

    /// Preformatted block (\preformatted{})
    Preformatted(String),

    /// Emphasis (\emph{})
    Emph(Vec<RdNode>),

    /// Strong/bold (\strong{} or \bold{})
    Strong(Vec<RdNode>),

    /// Hyperlink with URL and optional text (\href{url}{text})
    Href { url: String, text: Vec<RdNode> },

    /// Link to another topic (\link{} or \link[pkg]{topic})
    Link {
        /// Optional package name
        package: Option<String>,
        /// Topic name
        topic: String,
        /// Optional display text
        text: Option<Vec<RdNode>>,
    },

    /// URL (\url{})
    Url(String),

    /// Email (\email{})
    Email(String),

    /// File path (\file{})
    File(Vec<RdNode>),

    /// Package name (\pkg{})
    Pkg(String),

    /// Inline equation (\eqn{latex}{ascii})
    Eqn {
        latex: String,
        ascii: Option<String>,
    },

    /// Display equation (\deqn{latex}{ascii})
    Deqn {
        latex: String,
        ascii: Option<String>,
    },

    /// S-expression for dynamic content (\Sexpr{})
    Sexpr {
        options: Option<String>,
        code: String,
    },

    /// Conditional content (\if{format}{content})
    If {
        format: String,
        content: Vec<RdNode>,
    },

    /// Conditional content with else (\ifelse{format}{then}{else})
    IfElse {
        format: String,
        then_content: Vec<RdNode>,
        else_content: Vec<RdNode>,
    },

    /// Special characters
    Special(SpecialChar),

    /// Macro/command not specifically handled
    Macro {
        name: String,
        args: Vec<Vec<RdNode>>,
    },

    /// Line break (\cr)
    LineBreak,

    /// Tab character (\tab)
    Tab,

    /// Raw output for specific formats (\out{})
    Out(String),

    /// Figure/image (\figure{file}{options})
    Figure {
        file: String,
        options: Option<String>,
    },

    /// S3 method declaration in usage (\method{func}{class})
    Method { generic: String, class: String },

    /// S4 method declaration in usage (\S4method{func}{signature})
    S4Method { generic: String, signature: String },

    /// Sample code (\samp{})
    Samp(Vec<RdNode>),

    /// Single quote (\sQuote{})
    SQuote(Vec<RdNode>),

    /// Double quote (\dQuote{})
    DQuote(Vec<RdNode>),

    /// Acronym (\acronym{})
    Acronym(String),

    /// Definition (\dfn{})
    Dfn(Vec<RdNode>),

    /// Option name (\option{})
    Option(String),

    /// Keyboard input (\kbd{})
    Kbd(Vec<RdNode>),

    /// Variable name (\var{})
    Var(String),

    /// Environment variable (\env{})
    Env(String),

    /// Command line command (\command{})
    Command(String),
}

/// Description list item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DescribeItem {
    /// Term being described
    pub term: Vec<RdNode>,
    /// Description of the term
    pub description: Vec<RdNode>,
}

/// Special characters in Rd
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpecialChar {
    /// \R - R logo
    R,
    /// \dots or \ldots - ellipsis (...)
    Dots,
    /// Left brace \{
    LeftBrace,
    /// Right brace \}
    RightBrace,
    /// Backslash \\
    Backslash,
    /// Percent \%
    Percent,
    /// En-dash \enc{–}{--}
    EnDash,
    /// Em-dash \enc{—}{---}
    EmDash,
    /// Left single quote '
    Lsqb,
    /// Right single quote '
    Rsqb,
    /// Left double quote "
    Ldqb,
    /// Right double quote "
    Rdqb,
}

impl RdDocument {
    /// Create a new empty document
    pub fn new() -> Self {
        Self { sections: vec![] }
    }

    /// Get a section by tag
    pub fn get_section(&self, tag: &SectionTag) -> Option<&RdSection> {
        self.sections.iter().find(|s| &s.tag == tag)
    }

    /// Get all sections with a specific tag (e.g., multiple \alias)
    pub fn get_sections(&self, tag: &SectionTag) -> Vec<&RdSection> {
        self.sections.iter().filter(|s| &s.tag == tag).collect()
    }
}

impl Default for RdDocument {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_tag_from_str() {
        assert_eq!(SectionTag::from_str("name"), SectionTag::Name);
        assert_eq!(SectionTag::from_str("NAME"), SectionTag::Name);
        assert_eq!(SectionTag::from_str("description"), SectionTag::Description);
        assert_eq!(
            SectionTag::from_str("custom"),
            SectionTag::Unknown("custom".to_string())
        );
    }

    #[test]
    fn test_document_get_section() {
        let doc = RdDocument {
            sections: vec![
                RdSection {
                    tag: SectionTag::Name,
                    content: vec![RdNode::Text("test".to_string())],
                },
                RdSection {
                    tag: SectionTag::Title,
                    content: vec![RdNode::Text("Test Title".to_string())],
                },
            ],
        };

        assert!(doc.get_section(&SectionTag::Name).is_some());
        assert!(doc.get_section(&SectionTag::Description).is_none());
    }

    #[test]
    fn test_serialize_node() {
        let node = RdNode::Href {
            url: "https://example.com".to_string(),
            text: vec![RdNode::Text("Example".to_string())],
        };

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("href"));
        assert!(json.contains("https://example.com"));
    }
}
