//! mdast (Markdown Abstract Syntax Tree) types
//!
//! A subset of mdast nodes needed for Markdown generation.
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

    pub fn code_with_meta(
        lang: Option<String>,
        meta: Option<String>,
        value: impl Into<String>,
    ) -> Self {
        Node::Code(Code {
            lang,
            meta,
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

    pub fn link_with_title(
        url: impl Into<String>,
        title: impl Into<String>,
        children: Vec<Node>,
    ) -> Self {
        Node::Link(Link {
            url: url.into(),
            title: Some(title.into()),
            children,
        })
    }

    pub fn image(url: impl Into<String>, alt: impl Into<String>) -> Self {
        Node::Image(Image {
            url: url.into(),
            title: None,
            alt: alt.into(),
        })
    }

    pub fn image_with_title(
        url: impl Into<String>,
        alt: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Node::Image(Image {
            url: url.into(),
            title: Some(title.into()),
            alt: alt.into(),
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

    pub fn ordered_list_from(start: u32, children: Vec<Node>) -> Self {
        Node::List(List {
            ordered: true,
            start: Some(start),
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

    pub fn table(align: Vec<Option<Align>>, children: Vec<Node>) -> Self {
        Node::Table(Table { align, children })
    }

    pub fn table_row(children: Vec<Node>) -> Self {
        Node::TableRow(TableRow { children })
    }

    pub fn table_cell(children: Vec<Node>) -> Self {
        Node::TableCell(TableCell { children })
    }

    pub fn definition_list(children: Vec<Node>) -> Self {
        Node::DefinitionList(DefinitionList { children })
    }

    pub fn definition_term(children: Vec<Node>) -> Self {
        Node::DefinitionTerm(DefinitionTerm { children })
    }

    pub fn definition_description(children: Vec<Node>) -> Self {
        Node::DefinitionDescription(DefinitionDescription { children })
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

    pub fn html(value: impl Into<String>) -> Self {
        Node::Html(Html {
            value: value.into(),
        })
    }

    pub fn blockquote(children: Vec<Node>) -> Self {
        Node::Blockquote(Blockquote { children })
    }

    pub fn thematic_break() -> Self {
        Node::ThematicBreak
    }

    pub fn line_break() -> Self {
        Node::Break
    }
}

impl Root {
    pub fn new(children: Vec<Node>) -> Self {
        Self { children }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_constructors() {
        let text = Node::text("hello");
        assert!(matches!(text, Node::Text(Text { value }) if value == "hello"));

        let heading = Node::heading(2, vec![Node::text("Title")]);
        assert!(matches!(heading, Node::Heading(Heading { depth: 2, .. })));

        let para = Node::paragraph(vec![Node::text("content")]);
        assert!(matches!(para, Node::Paragraph(_)));
    }

    #[test]
    fn test_code_constructors() {
        let code = Node::code(Some("rust".to_string()), "fn main() {}");
        if let Node::Code(c) = code {
            assert_eq!(c.lang, Some("rust".to_string()));
            assert_eq!(c.meta, None);
            assert_eq!(c.value, "fn main() {}");
        } else {
            panic!("Expected Code node");
        }

        let code_meta = Node::code_with_meta(
            Some("r".to_string()),
            Some("executable".to_string()),
            "x <- 1",
        );
        if let Node::Code(c) = code_meta {
            assert_eq!(c.meta, Some("executable".to_string()));
        } else {
            panic!("Expected Code node");
        }
    }

    #[test]
    fn test_list_constructors() {
        let unordered = Node::list(false, vec![Node::list_item(vec![Node::text("item")])]);
        if let Node::List(l) = unordered {
            assert!(!l.ordered);
            assert_eq!(l.start, None);
        } else {
            panic!("Expected List node");
        }

        let ordered = Node::ordered_list_from(5, vec![Node::list_item(vec![Node::text("item")])]);
        if let Node::List(l) = ordered {
            assert!(l.ordered);
            assert_eq!(l.start, Some(5));
        } else {
            panic!("Expected List node");
        }
    }

    #[test]
    fn test_link_constructors() {
        let link = Node::link("https://example.com", vec![Node::text("Example")]);
        if let Node::Link(l) = link {
            assert_eq!(l.url, "https://example.com");
            assert_eq!(l.title, None);
        } else {
            panic!("Expected Link node");
        }

        let link_titled = Node::link_with_title(
            "https://example.com",
            "Example Site",
            vec![Node::text("Example")],
        );
        if let Node::Link(l) = link_titled {
            assert_eq!(l.title, Some("Example Site".to_string()));
        } else {
            panic!("Expected Link node");
        }
    }

    #[test]
    fn test_image_constructors() {
        let img = Node::image("image.png", "An image");
        if let Node::Image(i) = img {
            assert_eq!(i.url, "image.png");
            assert_eq!(i.alt, "An image");
            assert_eq!(i.title, None);
        } else {
            panic!("Expected Image node");
        }

        let img_titled = Node::image_with_title("image.png", "An image", "Image Title");
        if let Node::Image(i) = img_titled {
            assert_eq!(i.title, Some("Image Title".to_string()));
        } else {
            panic!("Expected Image node");
        }
    }

    #[test]
    fn test_table_constructors() {
        let table = Node::table(
            vec![Some(Align::Left), Some(Align::Right)],
            vec![Node::table_row(vec![
                Node::table_cell(vec![Node::text("A")]),
                Node::table_cell(vec![Node::text("B")]),
            ])],
        );
        if let Node::Table(t) = table {
            assert_eq!(t.align.len(), 2);
            assert_eq!(t.children.len(), 1);
        } else {
            panic!("Expected Table node");
        }
    }

    #[test]
    fn test_definition_list_constructors() {
        let dl = Node::definition_list(vec![
            Node::definition_term(vec![Node::text("Term")]),
            Node::definition_description(vec![Node::text("Definition")]),
        ]);
        assert!(matches!(dl, Node::DefinitionList(_)));
    }

    #[test]
    fn test_serde_roundtrip() {
        let root = Root::new(vec![
            Node::heading(1, vec![Node::text("Title")]),
            Node::paragraph(vec![
                Node::text("Hello "),
                Node::emphasis(vec![Node::text("world")]),
            ]),
        ]);

        let json = serde_json::to_string(&root).unwrap();
        let parsed: Root = serde_json::from_str(&json).unwrap();
        assert_eq!(root, parsed);
    }
}
