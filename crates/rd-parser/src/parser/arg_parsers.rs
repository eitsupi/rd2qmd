use crate::ast::RdNode;
use crate::lexer::TokenKind;
use super::{ParseResult, Parser};

impl Parser {
    /// Parse optional argument in brackets [...]
    pub(super) fn parse_bracketed_arg(&mut self) -> ParseResult<String> {
        self.expect(&TokenKind::OpenBracket)?;
        let text = self.parse_text_until_close_bracket()?;
        self.expect(&TokenKind::CloseBracket)?;
        Ok(text)
    }

    /// Parse text until close bracket
    pub(super) fn parse_text_until_close_bracket(&mut self) -> ParseResult<String> {
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
    pub(super) fn parse_text_until_close_brace(&mut self) -> ParseResult<String> {
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
    pub(super) fn parse_text_arg(&mut self) -> ParseResult<String> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let text = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;
        Ok(text)
    }

    /// Parse inline nodes (can contain nested macros)
    pub(super) fn parse_inline_nodes(&mut self) -> ParseResult<Vec<RdNode>> {
        self.skip_whitespace();
        self.parse_braced_content()
    }

    /// Parse verbatim content in braces (no macro processing)
    pub(super) fn parse_verbatim_inline(&mut self) -> ParseResult<String> {
        self.parse_text_arg()
    }

    /// Parse preformatted/verbatim block
    pub(super) fn parse_verbatim_block(&mut self) -> ParseResult<String> {
        self.parse_text_arg()
    }
}
