//! Rd file lexer
//!
//! Tokenizes Rd (R Documentation) files into a stream of tokens
//! for the parser to consume.

use std::iter::Peekable;
use std::str::Chars;

/// A token in an Rd file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// The kind of token
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// Backslash introducing a macro (\)
    Backslash,
    /// Opening brace ({)
    OpenBrace,
    /// Closing brace (})
    CloseBrace,
    /// Opening bracket ([)
    OpenBracket,
    /// Closing bracket (])
    CloseBracket,
    /// Plain text content
    Text(String),
    /// Whitespace (spaces and tabs, not newlines)
    Whitespace(String),
    /// Newline (\n or \r\n)
    Newline,
    /// End of file
    Eof,
}

/// Source location span
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// Starting byte offset
    pub start: usize,
    /// Ending byte offset (exclusive)
    pub end: usize,
    /// Starting line (1-indexed)
    pub line: usize,
    /// Starting column (1-indexed)
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }
}

/// Lexer for Rd files
pub struct Lexer<'a> {
    #[allow(dead_code)]
    input: &'a str,
    chars: Peekable<Chars<'a>>,
    /// Current byte position
    pos: usize,
    /// Current line (1-indexed)
    line: usize,
    /// Current column (1-indexed)
    column: usize,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given input
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().peekable(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(input: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(input);
        let mut tokens = Vec::new();
        loop {
            let token = lexer.next_token();
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        tokens
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Token {
        // Skip comments (% to end of line)
        self.skip_comments();

        let start_pos = self.pos;
        let start_line = self.line;
        let start_col = self.column;

        let Some(ch) = self.peek() else {
            return Token {
                kind: TokenKind::Eof,
                span: Span::new(start_pos, start_pos, start_line, start_col),
            };
        };

        let kind = match ch {
            '\\' => {
                self.advance();
                // Check for escape sequences
                match self.peek() {
                    Some('{') => {
                        self.advance();
                        TokenKind::Text("{".to_string())
                    }
                    Some('}') => {
                        self.advance();
                        TokenKind::Text("}".to_string())
                    }
                    Some('%') => {
                        self.advance();
                        TokenKind::Text("%".to_string())
                    }
                    Some('\\') => {
                        self.advance();
                        TokenKind::Text("\\".to_string())
                    }
                    _ => TokenKind::Backslash,
                }
            }
            '{' => {
                self.advance();
                TokenKind::OpenBrace
            }
            '}' => {
                self.advance();
                TokenKind::CloseBrace
            }
            '[' => {
                self.advance();
                TokenKind::OpenBracket
            }
            ']' => {
                self.advance();
                TokenKind::CloseBracket
            }
            '\n' => {
                self.advance();
                TokenKind::Newline
            }
            '\r' => {
                self.advance();
                // Handle \r\n as single newline
                if self.peek() == Some('\n') {
                    self.advance();
                }
                TokenKind::Newline
            }
            ' ' | '\t' => {
                let ws = self.consume_whitespace();
                TokenKind::Whitespace(ws)
            }
            _ => {
                let text = self.consume_text();
                TokenKind::Text(text)
            }
        };

        Token {
            kind,
            span: Span::new(start_pos, self.pos, start_line, start_col),
        }
    }

    /// Peek at the next character without consuming it
    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    /// Advance to the next character
    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    /// Skip comments (% to end of line)
    fn skip_comments(&mut self) {
        while self.peek() == Some('%') {
            // Consume until end of line
            while let Some(ch) = self.peek() {
                if ch == '\n' || ch == '\r' {
                    break;
                }
                self.advance();
            }
            // Also consume the newline after the comment
            if self.peek() == Some('\r') {
                self.advance();
            }
            if self.peek() == Some('\n') {
                self.advance();
            }
        }
    }

    /// Consume whitespace (spaces and tabs)
    fn consume_whitespace(&mut self) -> String {
        let mut ws = String::new();
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' {
                ws.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        ws
    }

    /// Consume text until a special character
    ///
    /// FIXME: This function treats parentheses and other punctuation as part of text,
    /// which causes issues when parsing macros like `\dots)`. The `)` gets included
    /// in the macro name, resulting in `macro{name:"dots)"}` instead of the special
    /// character `\dots` followed by text `)`.
    ///
    /// According to parseRd.pdf, macro names should only consist of alphanumeric
    /// characters. A proper fix would require either:
    /// 1. Making the lexer context-aware (knowing when it's after a backslash)
    /// 2. Handling macro name termination in the parser
    /// 3. Tokenizing punctuation separately
    ///
    /// See snapshot test `nested` for an example of this behavior.
    fn consume_text(&mut self) -> String {
        let mut text = String::new();
        while let Some(ch) = self.peek() {
            match ch {
                '\\' | '{' | '}' | '[' | ']' | '\n' | '\r' | '%' | ' ' | '\t' => break,
                _ => {
                    text.push(ch);
                    self.advance();
                }
            }
        }
        text
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.next_token();
        if token.kind == TokenKind::Eof {
            None
        } else {
            Some(token)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let tokens = Lexer::tokenize("");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn test_simple_text() {
        let tokens = Lexer::tokenize("hello");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("hello".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_macro() {
        let tokens = Lexer::tokenize("\\name{test}");
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].kind, TokenKind::Backslash);
        assert_eq!(tokens[1].kind, TokenKind::Text("name".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[3].kind, TokenKind::Text("test".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn test_escape_sequences() {
        let tokens = Lexer::tokenize("\\{\\}\\%\\\\");
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Text("{".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Text("}".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Text("%".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Text("\\".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn test_comment() {
        let tokens = Lexer::tokenize("before\n% comment\nafter");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::Text("before".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Newline);
        // Comment is skipped, including its newline
        assert_eq!(tokens[2].kind, TokenKind::Text("after".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn test_optional_arg() {
        let tokens = Lexer::tokenize("\\link[pkg]{topic}");
        assert_eq!(tokens.len(), 9);
        assert_eq!(tokens[0].kind, TokenKind::Backslash);
        assert_eq!(tokens[1].kind, TokenKind::Text("link".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::OpenBracket);
        assert_eq!(tokens[3].kind, TokenKind::Text("pkg".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::CloseBracket);
        assert_eq!(tokens[5].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[6].kind, TokenKind::Text("topic".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[8].kind, TokenKind::Eof);
    }

    #[test]
    fn test_whitespace() {
        let tokens = Lexer::tokenize("hello world");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::Text("hello".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Whitespace(" ".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Text("world".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn test_span_tracking() {
        let tokens = Lexer::tokenize("ab\ncd");
        assert_eq!(tokens[0].span.line, 1);
        assert_eq!(tokens[0].span.column, 1);
        assert_eq!(tokens[1].span.line, 1);
        assert_eq!(tokens[1].span.column, 3);
        assert_eq!(tokens[2].span.line, 2);
        assert_eq!(tokens[2].span.column, 1);
    }

    #[test]
    fn test_real_rd_snippet() {
        let input = r#"\name{test}
\title{Test Function}
"#;
        let tokens = Lexer::tokenize(input);
        // Should parse without panic
        assert!(tokens.len() > 1);
        assert!(matches!(
            tokens.last(),
            Some(Token {
                kind: TokenKind::Eof,
                ..
            })
        ));
    }
}
