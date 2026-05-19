//! Rd file parser
//!
//! Recursive descent parser that converts a token stream into an Rd AST.

use crate::ast::{RdDocument, RdNode, RdSection, SectionTag};
use crate::lexer::{Lexer, Token, TokenKind};
use thiserror::Error;

mod arg_parsers;
mod macro_dispatch;
mod node_parsers;
#[cfg(test)]
mod tests;

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
    pub(in crate::parser) tokens: Vec<Token>,
    pub(in crate::parser) pos: usize,
    /// When true, bare `{...}` in content is treated as literal text rather than Rd grouping.
    /// Used for R code contexts like `\examples{}` and `\usage{}` where braces are R syntax.
    pub(in crate::parser) preserve_braces: bool,
}

impl Parser {
    /// Create a new parser from source text
    pub fn new(source: &str) -> Self {
        Self {
            tokens: Lexer::tokenize(source),
            pos: 0,
            preserve_braces: false,
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

        let tag = SectionTag::parse(&name);

        // Parse section content in braces
        self.skip_whitespace();
        if !self.check(&TokenKind::OpenBrace) {
            // Some sections might not have braces (like \keyword)
            return Ok(Some(RdSection {
                tag,
                content: vec![],
            }));
        }

        // R code sections preserve literal braces (R syntax) rather than treating them
        // as Rd text grouping.
        let prev_preserve = self.preserve_braces;
        if matches!(tag, SectionTag::Usage | SectionTag::Examples) {
            self.preserve_braces = true;
        }
        let content = self.parse_braced_content()?;
        self.preserve_braces = prev_preserve;

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
    pub(in crate::parser) fn parse_braced_content(&mut self) -> ParseResult<Vec<RdNode>> {
        self.expect(&TokenKind::OpenBrace)?;
        let content = self.parse_content_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;
        Ok(content)
    }

    /// Parse content until we hit a closing brace (at the same nesting level)
    pub(in crate::parser) fn parse_content_until_close_brace(
        &mut self,
    ) -> ParseResult<Vec<RdNode>> {
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
                    if self.preserve_braces {
                        // In R code contexts (examples, usage), braces are literal R syntax.
                        current_text.push('{');
                        self.advance();
                        let inner = self.parse_content_until_close_brace()?;
                        self.expect(&TokenKind::CloseBrace)?;
                        for inner_node in inner {
                            match inner_node {
                                RdNode::Text(s) => current_text.push_str(&s),
                                other => {
                                    if !current_text.is_empty() {
                                        nodes.push(RdNode::Text(std::mem::take(&mut current_text)));
                                    }
                                    nodes.push(other);
                                }
                            }
                        }
                        current_text.push('}');
                    } else {
                        // In text contexts, nested braces are Rd grouping: unwrap inner content.
                        if !current_text.is_empty() {
                            nodes.push(RdNode::Text(std::mem::take(&mut current_text)));
                        }
                        self.advance();
                        let inner = self.parse_content_until_close_brace()?;
                        self.expect(&TokenKind::CloseBrace)?;
                        nodes.extend(inner);
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

    // -------------------------------------------------------------------------
    // Token navigation utilities
    // -------------------------------------------------------------------------

    pub(in crate::parser) fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    pub(in crate::parser) fn peek_kind(&self) -> TokenKind {
        self.peek()
            .map(|t| t.kind.clone())
            .unwrap_or(TokenKind::Eof)
    }

    pub(in crate::parser) fn advance(&mut self) -> Option<&Token> {
        if self.pos < self.tokens.len() {
            let token = &self.tokens[self.pos];
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    pub(in crate::parser) fn check(&self, kind: &TokenKind) -> bool {
        self.peek().map(|t| &t.kind == kind).unwrap_or(false)
    }

    pub(in crate::parser) fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    pub(in crate::parser) fn expect(&mut self, kind: &TokenKind) -> ParseResult<&Token> {
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

    /// Consume an immediately adjacent empty `{}` if the next two tokens are
    /// `OpenBrace` then `CloseBrace`. Used to silently drop the common Rd
    /// idiom of terminating zero-arg macros with `{}` (e.g. `\dots{}`).
    pub(in crate::parser) fn consume_optional_empty_braces(&mut self) {
        if self.check(&TokenKind::OpenBrace)
            && self.pos + 1 < self.tokens.len()
            && self.tokens[self.pos + 1].kind == TokenKind::CloseBrace
        {
            self.advance(); // {
            self.advance(); // }
        }
    }

    pub(in crate::parser) fn skip_whitespace(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Whitespace(_)) {
            self.advance();
        }
    }

    pub(in crate::parser) fn skip_whitespace_and_newlines(&mut self) {
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
