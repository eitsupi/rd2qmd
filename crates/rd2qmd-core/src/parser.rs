//! Rd file parser
//!
//! Recursive descent parser that converts a token stream into an Rd AST.

use crate::ast::{DescribeItem, RdDocument, RdNode, RdSection, SectionTag, SpecialChar};
use crate::lexer::{Lexer, Token, TokenKind};
use thiserror::Error;

/// Parser errors
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unexpected token at line {line}, column {col}: expected {expected}, found {found}")]
    UnexpectedToken {
        expected: String,
        found: String,
        line: usize,
        col: usize,
    },

    #[error("Unexpected end of file")]
    UnexpectedEof,

    #[error("Unknown macro: \\{name}")]
    UnknownMacro { name: String },

    #[error("Invalid macro arguments for \\{name}")]
    InvalidMacroArgs { name: String },
}

/// Parse result type
pub type ParseResult<T> = Result<T, ParseError>;

/// Rd file parser
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Create a new parser from source text
    pub fn new(source: &str) -> Self {
        Self {
            tokens: Lexer::tokenize(source),
            pos: 0,
        }
    }

    /// Parse the entire document
    pub fn parse(&mut self) -> ParseResult<RdDocument> {
        let mut sections = Vec::new();

        self.skip_whitespace_and_newlines();

        while !self.is_at_end() {
            if self.check(&TokenKind::Backslash) {
                if let Some(section) = self.parse_section()? {
                    sections.push(section);
                }
            } else {
                // Skip unexpected tokens at top level
                self.advance();
            }
            self.skip_whitespace_and_newlines();
        }

        Ok(RdDocument { sections })
    }

    /// Parse a top-level section
    fn parse_section(&mut self) -> ParseResult<Option<RdSection>> {
        self.expect(&TokenKind::Backslash)?;

        let name = self.parse_macro_name()?;

        // Handle special \section{title}{content} form
        if name == "section" {
            return self.parse_custom_section();
        }

        let tag = SectionTag::from_str(&name);

        // Parse section content in braces
        self.skip_whitespace();
        if !self.check(&TokenKind::OpenBrace) {
            // Some sections might not have braces (like \keyword)
            return Ok(Some(RdSection {
                tag,
                content: vec![],
            }));
        }

        let content = self.parse_braced_content()?;

        Ok(Some(RdSection { tag, content }))
    }

    /// Parse \section{title}{content}
    fn parse_custom_section(&mut self) -> ParseResult<Option<RdSection>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let title = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        let content = self.parse_braced_content()?;

        Ok(Some(RdSection {
            tag: SectionTag::Section(title),
            content,
        }))
    }

    /// Parse content within braces
    fn parse_braced_content(&mut self) -> ParseResult<Vec<RdNode>> {
        self.expect(&TokenKind::OpenBrace)?;
        let content = self.parse_content_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;
        Ok(content)
    }

    /// Parse content until we hit a closing brace (at the same nesting level)
    fn parse_content_until_close_brace(&mut self) -> ParseResult<Vec<RdNode>> {
        let mut nodes = Vec::new();
        let mut current_text = String::new();

        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            match self.peek_kind() {
                TokenKind::Backslash => {
                    // Flush accumulated text
                    if !current_text.is_empty() {
                        nodes.push(RdNode::Text(std::mem::take(&mut current_text)));
                    }
                    if let Some(node) = self.parse_macro()? {
                        nodes.push(node);
                    }
                }
                TokenKind::OpenBrace => {
                    // Nested braces - treat as text group
                    if !current_text.is_empty() {
                        nodes.push(RdNode::Text(std::mem::take(&mut current_text)));
                    }
                    self.advance();
                    let inner = self.parse_content_until_close_brace()?;
                    self.expect(&TokenKind::CloseBrace)?;
                    nodes.extend(inner);
                }
                TokenKind::Text(s) => {
                    current_text.push_str(&s);
                    self.advance();
                }
                TokenKind::Whitespace(ws) => {
                    current_text.push_str(&ws);
                    self.advance();
                }
                TokenKind::Newline => {
                    current_text.push('\n');
                    self.advance();
                }
                TokenKind::OpenBracket => {
                    current_text.push('[');
                    self.advance();
                }
                TokenKind::CloseBracket => {
                    current_text.push(']');
                    self.advance();
                }
                TokenKind::CloseBrace | TokenKind::Eof => break,
            }
        }

        // Flush remaining text
        if !current_text.is_empty() {
            nodes.push(RdNode::Text(current_text));
        }

        Ok(nodes)
    }

    /// Parse a macro (after seeing backslash)
    fn parse_macro(&mut self) -> ParseResult<Option<RdNode>> {
        self.expect(&TokenKind::Backslash)?;

        let name = self.parse_macro_name()?;

        // Handle special characters (no braces needed)
        match name.as_str() {
            "R" => return Ok(Some(RdNode::Special(SpecialChar::R))),
            "dots" | "ldots" => return Ok(Some(RdNode::Special(SpecialChar::Dots))),
            "cr" => return Ok(Some(RdNode::LineBreak)),
            "tab" => return Ok(Some(RdNode::Tab)),
            _ => {}
        }

        // Most macros require braces
        self.skip_whitespace();

        // Check for optional argument [...]
        let opt_arg = if self.check(&TokenKind::OpenBracket) {
            Some(self.parse_bracketed_arg()?)
        } else {
            None
        };

        // Parse based on macro name
        match name.as_str() {
            // Block elements
            "itemize" => self.parse_list(false),
            "enumerate" => self.parse_list(true),
            "describe" => self.parse_describe(),
            "tabular" => self.parse_tabular(),
            "preformatted" => self
                .parse_verbatim_block()
                .map(|s| Some(RdNode::Preformatted(s))),
            "subsection" => self.parse_subsection(),

            // Inline elements with content
            "code" => self.parse_inline_nodes().map(|n| Some(RdNode::Code(n))),
            "emph" => self.parse_inline_nodes().map(|n| Some(RdNode::Emph(n))),
            "strong" | "bold" => self.parse_inline_nodes().map(|n| Some(RdNode::Strong(n))),
            "samp" => self.parse_inline_nodes().map(|n| Some(RdNode::Samp(n))),
            "file" => self.parse_inline_nodes().map(|n| Some(RdNode::File(n))),
            "dfn" => self.parse_inline_nodes().map(|n| Some(RdNode::Dfn(n))),
            "kbd" => self.parse_inline_nodes().map(|n| Some(RdNode::Kbd(n))),
            "sQuote" => self.parse_inline_nodes().map(|n| Some(RdNode::SQuote(n))),
            "dQuote" => self.parse_inline_nodes().map(|n| Some(RdNode::DQuote(n))),

            // Inline elements with text content
            "verb" => self.parse_verbatim_inline().map(|s| Some(RdNode::Verb(s))),
            "url" => self.parse_text_arg().map(|s| Some(RdNode::Url(s))),
            "email" => self.parse_text_arg().map(|s| Some(RdNode::Email(s))),
            "pkg" => self.parse_text_arg().map(|s| Some(RdNode::Pkg(s))),
            "var" => self.parse_text_arg().map(|s| Some(RdNode::Var(s))),
            "env" => self.parse_text_arg().map(|s| Some(RdNode::Env(s))),
            "option" => self.parse_text_arg().map(|s| Some(RdNode::Option(s))),
            "command" => self.parse_text_arg().map(|s| Some(RdNode::Command(s))),
            "acronym" => self.parse_text_arg().map(|s| Some(RdNode::Acronym(s))),

            // Link-like elements
            "href" => self.parse_href(),
            "link" => self.parse_link(opt_arg),
            "Sexpr" => self.parse_sexpr(opt_arg),

            // Equations
            "eqn" => self.parse_equation(false),
            "deqn" => self.parse_equation(true),

            // Conditionals
            "if" => self.parse_if(),
            "ifelse" => self.parse_ifelse(),
            "out" => self.parse_verbatim_inline().map(|s| Some(RdNode::Out(s))),

            // Method declarations (in \usage)
            "method" => self.parse_method(),
            "S4method" => self.parse_s4method(),

            // Item (in lists)
            "item" => self.parse_item(),

            // Figure
            "figure" => self.parse_figure(opt_arg),

            // Unknown macro - store generically
            _ => self.parse_generic_macro(&name),
        }
    }

    /// Parse macro name (text following backslash)
    fn parse_macro_name(&mut self) -> ParseResult<String> {
        match self.peek_kind() {
            TokenKind::Text(s) => {
                let name = s.clone();
                self.advance();
                Ok(name)
            }
            // Special single-character escapes
            _ => Ok(String::new()),
        }
    }

    /// Parse optional argument in brackets [...]
    fn parse_bracketed_arg(&mut self) -> ParseResult<String> {
        self.expect(&TokenKind::OpenBracket)?;
        let text = self.parse_text_until_close_bracket()?;
        self.expect(&TokenKind::CloseBracket)?;
        Ok(text)
    }

    /// Parse text until close bracket
    fn parse_text_until_close_bracket(&mut self) -> ParseResult<String> {
        let mut text = String::new();
        while !self.check(&TokenKind::CloseBracket) && !self.is_at_end() {
            match self.peek_kind() {
                TokenKind::Text(s) => {
                    text.push_str(&s);
                    self.advance();
                }
                TokenKind::Whitespace(ws) => {
                    text.push_str(&ws);
                    self.advance();
                }
                TokenKind::Backslash => {
                    text.push('\\');
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        Ok(text)
    }

    /// Parse text until close brace (simple text, no macro processing)
    fn parse_text_until_close_brace(&mut self) -> ParseResult<String> {
        let mut text = String::new();
        let mut depth = 0;
        while !self.is_at_end() {
            match self.peek_kind() {
                TokenKind::OpenBrace => {
                    depth += 1;
                    text.push('{');
                    self.advance();
                }
                TokenKind::CloseBrace => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    text.push('}');
                    self.advance();
                }
                TokenKind::Text(s) => {
                    text.push_str(&s);
                    self.advance();
                }
                TokenKind::Whitespace(ws) => {
                    text.push_str(&ws);
                    self.advance();
                }
                TokenKind::Newline => {
                    text.push('\n');
                    self.advance();
                }
                TokenKind::Backslash => {
                    text.push('\\');
                    self.advance();
                }
                TokenKind::OpenBracket => {
                    text.push('[');
                    self.advance();
                }
                TokenKind::CloseBracket => {
                    text.push(']');
                    self.advance();
                }
                TokenKind::Eof => break,
            }
        }
        Ok(text)
    }

    /// Parse a simple text argument {text}
    fn parse_text_arg(&mut self) -> ParseResult<String> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let text = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;
        Ok(text)
    }

    /// Parse inline nodes (can contain nested macros)
    fn parse_inline_nodes(&mut self) -> ParseResult<Vec<RdNode>> {
        self.skip_whitespace();
        self.parse_braced_content()
    }

    /// Parse verbatim content in braces (no macro processing)
    fn parse_verbatim_inline(&mut self) -> ParseResult<String> {
        self.parse_text_arg()
    }

    /// Parse preformatted/verbatim block
    fn parse_verbatim_block(&mut self) -> ParseResult<String> {
        self.parse_text_arg()
    }

    /// Parse \itemize or \enumerate
    fn parse_list(&mut self, _numbered: bool) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;

        let mut items = Vec::new();
        self.skip_whitespace_and_newlines();

        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            if self.check(&TokenKind::Backslash) {
                // Look for \item
                let pos = self.pos;
                self.advance(); // consume backslash
                if let TokenKind::Text(name) = self.peek_kind() {
                    if name == "item" {
                        self.advance(); // consume "item"
                        if let Some(item) = self.parse_item()? {
                            items.push(item);
                        }
                        continue;
                    }
                }
                // Not an item, restore position
                self.pos = pos;
            }
            self.advance();
            self.skip_whitespace_and_newlines();
        }

        self.expect(&TokenKind::CloseBrace)?;

        if _numbered {
            Ok(Some(RdNode::Enumerate(items)))
        } else {
            Ok(Some(RdNode::Itemize(items)))
        }
    }

    /// Parse \item - content until next \item or closing brace
    fn parse_item(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();

        // Check for optional label {label}
        let label = if self.check(&TokenKind::OpenBrace) {
            Some(self.parse_braced_content()?)
        } else {
            None
        };

        // Parse content until next \item or }
        let mut content = Vec::new();
        let mut current_text = String::new();

        while !self.is_at_end() {
            // Check for end of item
            if self.check(&TokenKind::CloseBrace) {
                break;
            }
            if self.check(&TokenKind::Backslash) {
                // Peek ahead to check for \item
                let next_pos = self.pos + 1;
                if next_pos < self.tokens.len() {
                    if let TokenKind::Text(name) = &self.tokens[next_pos].kind {
                        if name == "item" {
                            break;
                        }
                    }
                }
            }

            match self.peek_kind() {
                TokenKind::Backslash => {
                    if !current_text.is_empty() {
                        content.push(RdNode::Text(std::mem::take(&mut current_text)));
                    }
                    if let Some(node) = self.parse_macro()? {
                        content.push(node);
                    }
                }
                TokenKind::Text(s) => {
                    current_text.push_str(&s);
                    self.advance();
                }
                TokenKind::Whitespace(ws) => {
                    current_text.push_str(&ws);
                    self.advance();
                }
                TokenKind::Newline => {
                    current_text.push('\n');
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }

        if !current_text.is_empty() {
            content.push(RdNode::Text(current_text));
        }

        Ok(Some(RdNode::Item { label, content }))
    }

    /// Parse \describe (description list)
    fn parse_describe(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;

        let mut items = Vec::new();
        self.skip_whitespace_and_newlines();

        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            if self.check(&TokenKind::Backslash) {
                let pos = self.pos;
                self.advance();
                if let TokenKind::Text(name) = self.peek_kind() {
                    if name == "item" {
                        self.advance();
                        self.skip_whitespace();
                        // \item{term}{description}
                        let term = self.parse_braced_content()?;
                        self.skip_whitespace();
                        let description = self.parse_braced_content()?;
                        items.push(DescribeItem { term, description });
                        self.skip_whitespace_and_newlines();
                        continue;
                    }
                }
                self.pos = pos;
            }
            self.advance();
        }

        self.expect(&TokenKind::CloseBrace)?;
        Ok(Some(RdNode::Describe(items)))
    }

    /// Parse \tabular{alignment}{content}
    fn parse_tabular(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let alignment = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;

        // Parse table content - cells separated by \tab, rows by \cr
        let mut rows: Vec<Vec<Vec<RdNode>>> = Vec::new();
        let mut current_row: Vec<Vec<RdNode>> = Vec::new();
        let mut current_cell: Vec<RdNode> = Vec::new();
        let mut current_text = String::new();

        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            match self.peek_kind() {
                TokenKind::Backslash => {
                    let pos = self.pos;
                    self.advance();
                    match self.peek_kind() {
                        TokenKind::Text(name) if name == "tab" => {
                            self.advance();
                            if !current_text.is_empty() {
                                current_cell.push(RdNode::Text(std::mem::take(&mut current_text)));
                            }
                            current_row.push(std::mem::take(&mut current_cell));
                        }
                        TokenKind::Text(name) if name == "cr" => {
                            self.advance();
                            if !current_text.is_empty() {
                                current_cell.push(RdNode::Text(std::mem::take(&mut current_text)));
                            }
                            current_row.push(std::mem::take(&mut current_cell));
                            rows.push(std::mem::take(&mut current_row));
                        }
                        _ => {
                            self.pos = pos;
                            if !current_text.is_empty() {
                                current_cell.push(RdNode::Text(std::mem::take(&mut current_text)));
                            }
                            if let Some(node) = self.parse_macro()? {
                                current_cell.push(node);
                            }
                        }
                    }
                }
                TokenKind::Text(s) => {
                    current_text.push_str(&s);
                    self.advance();
                }
                TokenKind::Whitespace(ws) => {
                    current_text.push_str(&ws);
                    self.advance();
                }
                TokenKind::Newline => {
                    current_text.push('\n');
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }

        // Flush remaining content
        if !current_text.is_empty() {
            current_cell.push(RdNode::Text(current_text));
        }
        if !current_cell.is_empty() {
            current_row.push(current_cell);
        }
        if !current_row.is_empty() {
            rows.push(current_row);
        }

        self.expect(&TokenKind::CloseBrace)?;

        Ok(Some(RdNode::Tabular { alignment, rows }))
    }

    /// Parse \subsection{title}{content}
    fn parse_subsection(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        let title = self.parse_braced_content()?;

        self.skip_whitespace();
        let content = self.parse_braced_content()?;

        Ok(Some(RdNode::Subsection { title, content }))
    }

    /// Parse \href{url}{text}
    fn parse_href(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let url = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        let text = self.parse_braced_content()?;

        Ok(Some(RdNode::Href { url, text }))
    }

    /// Parse \link[pkg]{topic} or \link[=dest]{text}
    fn parse_link(&mut self, opt_arg: Option<String>) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        let topic_or_text = self.parse_braced_content()?;

        let (package, topic, text) = if let Some(opt) = opt_arg {
            if opt.starts_with('=') {
                // \link[=dest]{text} form
                let dest = opt[1..].to_string();
                (None, dest, Some(topic_or_text))
            } else {
                // \link[pkg]{topic} form
                let topic = topic_or_text
                    .first()
                    .map(|n| match n {
                        RdNode::Text(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                (Some(opt), topic, None)
            }
        } else {
            // \link{topic} form
            let topic = topic_or_text
                .first()
                .map(|n| match n {
                    RdNode::Text(s) => s.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            (None, topic, None)
        };

        Ok(Some(RdNode::Link {
            package,
            topic,
            text,
        }))
    }

    /// Parse \eqn{latex}{ascii} or \deqn{latex}{ascii}
    fn parse_equation(&mut self, display: bool) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let latex = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        // Optional ASCII alternative
        self.skip_whitespace();
        let ascii = if self.check(&TokenKind::OpenBrace) {
            self.advance();
            let ascii = self.parse_text_until_close_brace()?;
            self.expect(&TokenKind::CloseBrace)?;
            Some(ascii)
        } else {
            None
        };

        if display {
            Ok(Some(RdNode::Deqn { latex, ascii }))
        } else {
            Ok(Some(RdNode::Eqn { latex, ascii }))
        }
    }

    /// Parse \Sexpr[options]{code}
    fn parse_sexpr(&mut self, opt_arg: Option<String>) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let code = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        Ok(Some(RdNode::Sexpr {
            options: opt_arg,
            code,
        }))
    }

    /// Parse \if{format}{content}
    fn parse_if(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let format = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        let content = self.parse_braced_content()?;

        Ok(Some(RdNode::If { format, content }))
    }

    /// Parse \ifelse{format}{then}{else}
    fn parse_ifelse(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let format = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        let then_content = self.parse_braced_content()?;

        self.skip_whitespace();
        let else_content = self.parse_braced_content()?;

        Ok(Some(RdNode::IfElse {
            format,
            then_content,
            else_content,
        }))
    }

    /// Parse \method{generic}{class}
    fn parse_method(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let generic = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let class = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        Ok(Some(RdNode::Method { generic, class }))
    }

    /// Parse \S4method{generic}{signature}
    fn parse_s4method(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let generic = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let signature = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        Ok(Some(RdNode::S4Method { generic, signature }))
    }

    /// Parse \figure{file}{options}
    fn parse_figure(&mut self, opt_arg: Option<String>) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let file = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        Ok(Some(RdNode::Figure {
            file,
            options: opt_arg,
        }))
    }

    /// Parse unknown macro generically
    fn parse_generic_macro(&mut self, name: &str) -> ParseResult<Option<RdNode>> {
        let mut args = Vec::new();

        self.skip_whitespace();
        while self.check(&TokenKind::OpenBrace) {
            let content = self.parse_braced_content()?;
            args.push(content);
            self.skip_whitespace();
        }

        Ok(Some(RdNode::Macro {
            name: name.to_string(),
            args,
        }))
    }

    // Helper methods

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> TokenKind {
        self.peek()
            .map(|t| t.kind.clone())
            .unwrap_or(TokenKind::Eof)
    }

    fn advance(&mut self) -> Option<&Token> {
        if self.pos < self.tokens.len() {
            let token = &self.tokens[self.pos];
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek().map(|t| &t.kind == kind).unwrap_or(false)
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn expect(&mut self, kind: &TokenKind) -> ParseResult<&Token> {
        if self.check(kind) {
            Ok(self.advance().unwrap())
        } else {
            let token = self.peek();
            Err(ParseError::UnexpectedToken {
                expected: format!("{:?}", kind),
                found: format!("{:?}", self.peek_kind()),
                line: token.map(|t| t.span.line).unwrap_or(0),
                col: token.map(|t| t.span.column).unwrap_or(0),
            })
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Whitespace(_)) {
            self.advance();
        }
    }

    fn skip_whitespace_and_newlines(&mut self) {
        while matches!(
            self.peek_kind(),
            TokenKind::Whitespace(_) | TokenKind::Newline
        ) {
            self.advance();
        }
    }
}

/// Convenience function to parse Rd source
pub fn parse(source: &str) -> ParseResult<RdDocument> {
    let mut parser = Parser::new(source);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_document() {
        let doc = parse("").unwrap();
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn test_simple_section() {
        let doc = parse("\\name{test}").unwrap();
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].tag, SectionTag::Name);
    }

    #[test]
    fn test_multiple_sections() {
        let doc = parse("\\name{foo}\n\\title{Bar}").unwrap();
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.sections[0].tag, SectionTag::Name);
        assert_eq!(doc.sections[1].tag, SectionTag::Title);
    }

    #[test]
    fn test_inline_code() {
        let doc = parse("\\description{Use \\code{foo} here}").unwrap();
        assert_eq!(doc.sections.len(), 1);
        let content = &doc.sections[0].content;
        assert!(content.len() >= 3); // Text, Code, Text
    }

    #[test]
    fn test_href() {
        let doc = parse("\\description{\\href{https://example.com}{Example}}").unwrap();
        let content = &doc.sections[0].content;
        assert!(matches!(content[0], RdNode::Href { .. }));
    }

    #[test]
    fn test_itemize() {
        let doc = parse("\\details{\\itemize{\\item One\\item Two}}").unwrap();
        let content = &doc.sections[0].content;
        assert!(matches!(&content[0], RdNode::Itemize(_)));
    }

    #[test]
    fn test_subsection() {
        let doc = parse("\\details{\\subsection{Sub}{Content here}}").unwrap();
        let content = &doc.sections[0].content;
        assert!(matches!(&content[0], RdNode::Subsection { .. }));
    }

    #[test]
    fn test_special_chars() {
        let doc = parse("\\description{\\R and \\dots}").unwrap();
        let content = &doc.sections[0].content;
        assert!(
            content
                .iter()
                .any(|n| matches!(n, RdNode::Special(SpecialChar::R)))
        );
        assert!(
            content
                .iter()
                .any(|n| matches!(n, RdNode::Special(SpecialChar::Dots)))
        );
    }

    #[test]
    fn test_real_rd_file() {
        let source = r#"
\name{test}
\alias{test}
\title{Test Function}
\description{
This is a test with \code{inline code} and a \href{https://example.com}{link}.
}
\usage{
test(x, y = TRUE)
}
\arguments{
\item{x}{The first argument}
\item{y}{The second argument}
}
"#;
        let doc = parse(source).unwrap();
        assert!(doc.sections.len() >= 5);
    }
}
