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

    // ==========================================================================
    // Basic token type tests
    // ==========================================================================

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
    fn test_backslash_alone() {
        let tokens = Lexer::tokenize("\\");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Backslash);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_open_brace() {
        let tokens = Lexer::tokenize("{");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_close_brace() {
        let tokens = Lexer::tokenize("}");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_open_bracket() {
        let tokens = Lexer::tokenize("[");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::OpenBracket);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_close_bracket() {
        let tokens = Lexer::tokenize("]");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::CloseBracket);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_newline_lf() {
        let tokens = Lexer::tokenize("\n");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Newline);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_newline_crlf() {
        let tokens = Lexer::tokenize("\r\n");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Newline);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_newline_cr_only() {
        let tokens = Lexer::tokenize("\r");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Newline);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_whitespace_space() {
        let tokens = Lexer::tokenize("   ");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Whitespace("   ".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_whitespace_tab() {
        let tokens = Lexer::tokenize("\t\t");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Whitespace("\t\t".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_whitespace_mixed() {
        let tokens = Lexer::tokenize(" \t ");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Whitespace(" \t ".to_string()));
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

    // ==========================================================================
    // Escape sequence tests
    // ==========================================================================

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
    fn test_escape_brace_open() {
        let tokens = Lexer::tokenize("\\{");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("{".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_escape_brace_close() {
        let tokens = Lexer::tokenize("\\}");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("}".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_escape_percent() {
        let tokens = Lexer::tokenize("\\%");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("%".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_escape_backslash() {
        let tokens = Lexer::tokenize("\\\\");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("\\".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_escape_in_text() {
        let tokens = Lexer::tokenize("10\\% discount");
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Text("10".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Text("%".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Whitespace(" ".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Text("discount".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn test_consecutive_escapes() {
        let tokens = Lexer::tokenize("\\\\\\\\");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Text("\\".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Text("\\".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn test_backslash_followed_by_text() {
        // \n (backslash followed by 'n') is NOT an escape, it's a macro
        let tokens = Lexer::tokenize("\\n");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Backslash);
        assert_eq!(tokens[1].kind, TokenKind::Text("n".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    // ==========================================================================
    // Comment tests
    // ==========================================================================

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
    fn test_comment_at_start() {
        let tokens = Lexer::tokenize("% comment\ntext");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("text".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_comment_at_end() {
        let tokens = Lexer::tokenize("text\n% comment");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Text("text".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Newline);
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn test_comment_only() {
        let tokens = Lexer::tokenize("% just a comment");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn test_multiple_consecutive_comments() {
        let tokens = Lexer::tokenize("% first\n% second\n% third\ntext");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("text".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_comment_with_special_chars() {
        let tokens = Lexer::tokenize("% comment with \\macro{} and {braces}\ntext");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("text".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_escaped_percent_not_comment() {
        // "\% not a comment" -> % + " " + not + " " + a + " " + comment + Eof
        let tokens = Lexer::tokenize("\\% not a comment");
        assert_eq!(tokens.len(), 8);
        assert_eq!(tokens[0].kind, TokenKind::Text("%".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Whitespace(" ".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Text("not".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Whitespace(" ".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Text("a".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::Whitespace(" ".to_string()));
        assert_eq!(tokens[6].kind, TokenKind::Text("comment".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn test_comment_crlf() {
        let tokens = Lexer::tokenize("text\r\n% comment\r\nmore");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::Text("text".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Newline);
        assert_eq!(tokens[2].kind, TokenKind::Text("more".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    // ==========================================================================
    // Non-ASCII and multibyte character tests
    // ==========================================================================

    #[test]
    fn test_japanese_text() {
        let tokens = Lexer::tokenize("日本語テキスト");
        assert_eq!(tokens.len(), 2);
        assert_eq!(
            tokens[0].kind,
            TokenKind::Text("日本語テキスト".to_string())
        );
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_mixed_ascii_japanese() {
        let tokens = Lexer::tokenize("Hello世界");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("Hello世界".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_macro_with_japanese() {
        let tokens = Lexer::tokenize("\\title{日本語タイトル}");
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].kind, TokenKind::Backslash);
        assert_eq!(tokens[1].kind, TokenKind::Text("title".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::OpenBrace);
        assert_eq!(
            tokens[3].kind,
            TokenKind::Text("日本語タイトル".to_string())
        );
        assert_eq!(tokens[4].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn test_emoji() {
        let tokens = Lexer::tokenize("🎉🎊");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text("🎉🎊".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_unicode_accents() {
        let tokens = Lexer::tokenize("café résumé");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::Text("café".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Whitespace(" ".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Text("résumé".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    // ==========================================================================
    // Nested and complex brace tests
    // ==========================================================================

    #[test]
    fn test_nested_braces() {
        let tokens = Lexer::tokenize("{{inner}}");
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[1].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[2].kind, TokenKind::Text("inner".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[4].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn test_deeply_nested_braces() {
        let tokens = Lexer::tokenize("{{{a}}}");
        assert_eq!(tokens.len(), 8);
        assert_eq!(tokens[0].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[1].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[2].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[3].kind, TokenKind::Text("a".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[5].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[6].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn test_adjacent_braces() {
        let tokens = Lexer::tokenize("{a}{b}");
        assert_eq!(tokens.len(), 7);
        assert_eq!(tokens[0].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[1].kind, TokenKind::Text("a".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[3].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[4].kind, TokenKind::Text("b".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[6].kind, TokenKind::Eof);
    }

    #[test]
    fn test_unbalanced_braces_open() {
        // Lexer doesn't validate balance, just tokenizes
        let tokens = Lexer::tokenize("{{{");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[1].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[2].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn test_unbalanced_braces_close() {
        let tokens = Lexer::tokenize("}}}");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[1].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[2].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn test_nested_brackets() {
        let tokens = Lexer::tokenize("[[inner]]");
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].kind, TokenKind::OpenBracket);
        assert_eq!(tokens[1].kind, TokenKind::OpenBracket);
        assert_eq!(tokens[2].kind, TokenKind::Text("inner".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::CloseBracket);
        assert_eq!(tokens[4].kind, TokenKind::CloseBracket);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    // ==========================================================================
    // Span (position) tracking tests
    // ==========================================================================

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
    fn test_span_byte_offsets() {
        let tokens = Lexer::tokenize("abc");
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 3);
    }

    #[test]
    fn test_span_multibyte_byte_offsets() {
        // 日 is 3 bytes, 本 is 3 bytes
        let tokens = Lexer::tokenize("日本");
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 6); // 3 + 3 bytes
    }

    #[test]
    fn test_span_multibyte_columns() {
        // Column advances by 1 per character, not per byte
        let tokens = Lexer::tokenize("日本 text");
        assert_eq!(tokens[0].span.column, 1);
        assert_eq!(tokens[1].span.column, 3); // After 2 characters
        assert_eq!(tokens[2].span.column, 4);
    }

    #[test]
    fn test_span_across_newlines() {
        let tokens = Lexer::tokenize("a\nb\nc");
        assert_eq!(tokens[0].span, Span::new(0, 1, 1, 1)); // 'a'
        assert_eq!(tokens[1].span, Span::new(1, 2, 1, 2)); // '\n'
        assert_eq!(tokens[2].span, Span::new(2, 3, 2, 1)); // 'b'
        assert_eq!(tokens[3].span, Span::new(3, 4, 2, 2)); // '\n'
        assert_eq!(tokens[4].span, Span::new(4, 5, 3, 1)); // 'c'
    }

    #[test]
    fn test_span_crlf() {
        let tokens = Lexer::tokenize("a\r\nb");
        assert_eq!(tokens[0].span.line, 1);
        assert_eq!(tokens[0].span.column, 1);
        assert_eq!(tokens[1].span.start, 1);
        assert_eq!(tokens[1].span.end, 3); // \r\n is 2 bytes
        assert_eq!(tokens[2].span.line, 2);
        assert_eq!(tokens[2].span.column, 1);
    }

    #[test]
    fn test_span_whitespace() {
        let tokens = Lexer::tokenize("a   b");
        assert_eq!(tokens[0].span, Span::new(0, 1, 1, 1)); // 'a'
        assert_eq!(tokens[1].span, Span::new(1, 4, 1, 2)); // '   '
        assert_eq!(tokens[2].span, Span::new(4, 5, 1, 5)); // 'b'
    }

    #[test]
    fn test_span_eof_at_end() {
        let tokens = Lexer::tokenize("ab");
        assert_eq!(tokens[1].kind, TokenKind::Eof);
        assert_eq!(tokens[1].span.start, 2);
        assert_eq!(tokens[1].span.end, 2);
    }

    #[test]
    fn test_span_eof_empty_input() {
        let tokens = Lexer::tokenize("");
        assert_eq!(tokens[0].kind, TokenKind::Eof);
        assert_eq!(tokens[0].span, Span::new(0, 0, 1, 1));
    }

    // ==========================================================================
    // Edge case and stress tests
    // ==========================================================================

    #[test]
    fn test_long_text() {
        let long_text = "a".repeat(10000);
        let tokens = Lexer::tokenize(&long_text);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Text(long_text));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_many_tokens() {
        // Generate many alternating tokens
        let input = "{a}".repeat(1000);
        let tokens = Lexer::tokenize(&input);
        // Each {a} produces 3 tokens, plus Eof
        assert_eq!(tokens.len(), 3001);
    }

    #[test]
    fn test_only_special_chars() {
        let tokens = Lexer::tokenize("{}[]\\");
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[1].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[2].kind, TokenKind::OpenBracket);
        assert_eq!(tokens[3].kind, TokenKind::CloseBracket);
        assert_eq!(tokens[4].kind, TokenKind::Backslash);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn test_whitespace_only() {
        let tokens = Lexer::tokenize("   \t\t   ");
        assert_eq!(tokens.len(), 2);
        assert_eq!(
            tokens[0].kind,
            TokenKind::Whitespace("   \t\t   ".to_string())
        );
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn test_newlines_only() {
        let tokens = Lexer::tokenize("\n\n\n");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::Newline);
        assert_eq!(tokens[1].kind, TokenKind::Newline);
        assert_eq!(tokens[2].kind, TokenKind::Newline);
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn test_mixed_newline_styles() {
        let tokens = Lexer::tokenize("a\nb\r\nc\rd");
        assert_eq!(tokens.len(), 8);
        assert_eq!(tokens[0].kind, TokenKind::Text("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Newline); // \n
        assert_eq!(tokens[2].kind, TokenKind::Text("b".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Newline); // \r\n
        assert_eq!(tokens[4].kind, TokenKind::Text("c".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::Newline); // \r
        assert_eq!(tokens[6].kind, TokenKind::Text("d".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn test_consecutive_backslashes_and_macro() {
        // \\\\ produces two escaped backslashes, then \name is a macro
        let tokens = Lexer::tokenize("\\\\\\\\\\name");
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Text("\\".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Text("\\".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Backslash);
        assert_eq!(tokens[3].kind, TokenKind::Text("name".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn test_backslash_at_eof() {
        let tokens = Lexer::tokenize("text\\");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Text("text".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Backslash);
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    // ==========================================================================
    // Real Rd file patterns
    // ==========================================================================

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

    #[test]
    fn test_real_rd_with_usage() {
        let input = r#"\usage{
foo(x, y = 1)
}"#;
        let tokens = Lexer::tokenize(input);
        assert_eq!(tokens[0].kind, TokenKind::Backslash);
        assert_eq!(tokens[1].kind, TokenKind::Text("usage".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[3].kind, TokenKind::Newline);
    }

    #[test]
    fn test_real_rd_with_itemize() {
        let input = r#"\itemize{
  \item First
  \item Second
}"#;
        let tokens = Lexer::tokenize(input);
        // Verify it contains the expected macro structures
        let text_tokens: Vec<_> = tokens
            .iter()
            .filter_map(|t| {
                if let TokenKind::Text(s) = &t.kind {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(text_tokens.contains(&"itemize"));
        assert!(text_tokens.contains(&"item"));
        assert!(text_tokens.contains(&"First"));
        assert!(text_tokens.contains(&"Second"));
    }

    #[test]
    fn test_real_rd_with_code() {
        let input = r#"\code{x <- 1}"#;
        let tokens = Lexer::tokenize(input);
        assert_eq!(tokens[0].kind, TokenKind::Backslash);
        assert_eq!(tokens[1].kind, TokenKind::Text("code".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[3].kind, TokenKind::Text("x".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Whitespace(" ".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::Text("<-".to_string()));
    }

    #[test]
    fn test_real_rd_with_link() {
        // \link[base]{print} -> \ + link + [ + base + ] + { + print + } + Eof
        let input = r#"\link[base]{print}"#;
        let tokens = Lexer::tokenize(input);
        assert_eq!(tokens.len(), 9);
        assert_eq!(tokens[0].kind, TokenKind::Backslash);
        assert_eq!(tokens[1].kind, TokenKind::Text("link".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::OpenBracket);
        assert_eq!(tokens[3].kind, TokenKind::Text("base".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::CloseBracket);
        assert_eq!(tokens[5].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[6].kind, TokenKind::Text("print".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[8].kind, TokenKind::Eof);
    }

    // ==========================================================================
    // Iterator tests
    // ==========================================================================

    #[test]
    fn test_iterator_basic() {
        let lexer = Lexer::new("a b");
        let tokens: Vec<_> = lexer.collect();
        assert_eq!(tokens.len(), 3); // Iterator excludes Eof
        assert_eq!(tokens[0].kind, TokenKind::Text("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Whitespace(" ".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Text("b".to_string()));
    }

    #[test]
    fn test_iterator_empty() {
        let lexer = Lexer::new("");
        let tokens: Vec<_> = lexer.collect();
        assert_eq!(tokens.len(), 0); // Empty input yields no tokens from iterator
    }

    #[test]
    fn test_iterator_vs_tokenize() {
        let input = "\\macro{arg}";
        let iter_tokens: Vec<_> = Lexer::new(input).collect();
        let tokenize_tokens = Lexer::tokenize(input);

        // tokenize includes Eof, iterator doesn't
        assert_eq!(iter_tokens.len() + 1, tokenize_tokens.len());
        for (i, t) in iter_tokens.iter().enumerate() {
            assert_eq!(t.kind, tokenize_tokens[i].kind);
        }
    }
}
