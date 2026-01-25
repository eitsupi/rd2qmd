//! mdast (Markdown Abstract Syntax Tree) types
//!
//! A subset of mdast nodes needed for Rd to Markdown conversion.
//! Reference: https://github.com/syntax-tree/mdast

use serde::{Deserialize, Serialize};

/// Root node of an mdast document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Root {
    pub children: Vec<Node>,
}

/// An mdast node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Node {
    // Block nodes
    Heading(Heading),
    Paragraph(Paragraph),
    ThematicBreak,
    Blockquote(Blockquote),
    List(List),
    ListItem(ListItem),
    Code(Code),
    Table(Table),
    TableRow(TableRow),
    TableCell(TableCell),

    // Container for definition lists (not standard mdast, but useful)
    DefinitionList(DefinitionList),
    DefinitionTerm(DefinitionTerm),
    DefinitionDescription(DefinitionDescription),

    // Inline nodes
    Text(Text),
    Emphasis(Emphasis),
    Strong(Strong),
    InlineCode(InlineCode),
    Break,
    Link(Link),
    Image(Image),

    // Math (mdast extension)
    Math(Math),
    InlineMath(InlineMath),

    // HTML (for raw output)
    Html(Html),
}

/// Heading node (# to ######)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heading {
    pub depth: u8,
    pub children: Vec<Node>,
}

/// Paragraph node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    pub children: Vec<Node>,
}

/// Blockquote node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blockquote {
    pub children: Vec<Node>,
}

/// List node (ordered or unordered)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    pub ordered: bool,
    pub start: Option<u32>,
    pub spread: bool,
    pub children: Vec<Node>,
}

/// List item node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    pub spread: bool,
    pub children: Vec<Node>,
}

/// Code block node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Code {
    pub lang: Option<String>,
    pub meta: Option<String>,
    pub value: String,
}

/// Table node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub align: Vec<Option<Align>>,
    pub children: Vec<Node>,
}

/// Table row node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    pub children: Vec<Node>,
}

/// Table cell node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub children: Vec<Node>,
}

/// Table alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Definition list (extension)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefinitionList {
    pub children: Vec<Node>,
}

/// Definition term (extension)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefinitionTerm {
    pub children: Vec<Node>,
}

/// Definition description (extension)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefinitionDescription {
    pub children: Vec<Node>,
}

/// Text node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub value: String,
}

/// Emphasis node (*text* or _text_)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Emphasis {
    pub children: Vec<Node>,
}

/// Strong node (**text** or __text__)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Strong {
    pub children: Vec<Node>,
}

/// Inline code node (`code`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineCode {
    pub value: String,
}

/// Link node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
    pub title: Option<String>,
    pub children: Vec<Node>,
}

/// Image node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub url: String,
    pub title: Option<String>,
    pub alt: String,
}

/// Display math node ($$...$$)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Math {
    pub value: String,
}

/// Inline math node ($...$)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineMath {
    pub value: String,
}

/// Raw HTML node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Html {
    pub value: String,
}

// Convenience constructors
impl Node {
    pub fn text(s: impl Into<String>) -> Self {
        Node::Text(Text { value: s.into() })
    }

    pub fn paragraph(children: Vec<Node>) -> Self {
        Node::Paragraph(Paragraph { children })
    }

    pub fn heading(depth: u8, children: Vec<Node>) -> Self {
        Node::Heading(Heading { depth, children })
    }

    pub fn code(lang: Option<String>, value: impl Into<String>) -> Self {
        Node::Code(Code {
            lang,
            meta: None,
            value: value.into(),
        })
    }

    pub fn inline_code(value: impl Into<String>) -> Self {
        Node::InlineCode(InlineCode {
            value: value.into(),
        })
    }

    pub fn emphasis(children: Vec<Node>) -> Self {
        Node::Emphasis(Emphasis { children })
    }

    pub fn strong(children: Vec<Node>) -> Self {
        Node::Strong(Strong { children })
    }

    pub fn link(url: impl Into<String>, children: Vec<Node>) -> Self {
        Node::Link(Link {
            url: url.into(),
            title: None,
            children,
        })
    }

    pub fn list(ordered: bool, children: Vec<Node>) -> Self {
        Node::List(List {
            ordered,
            start: if ordered { Some(1) } else { None },
            spread: false,
            children,
        })
    }

    pub fn list_item(children: Vec<Node>) -> Self {
        Node::ListItem(ListItem {
            spread: false,
            children,
        })
    }

    pub fn math(value: impl Into<String>) -> Self {
        Node::Math(Math {
            value: value.into(),
        })
    }

    pub fn inline_math(value: impl Into<String>) -> Self {
        Node::InlineMath(InlineMath {
            value: value.into(),
        })
    }
}

impl Root {
    pub fn new(children: Vec<Node>) -> Self {
        Self { children }
    }
}
