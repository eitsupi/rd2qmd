use super::{ParseResult, Parser};
use crate::ast::{DescribeItem, FigureOptions, RdNode};
use crate::lexer::TokenKind;

impl Parser {
    /// Parse \itemize or \enumerate
    pub(super) fn parse_list(&mut self, _numbered: bool) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;

        let mut items = Vec::new();
        self.skip_whitespace_and_newlines();

        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            if self.check(&TokenKind::Backslash) {
                // Look for \item
                let pos = self.pos;
                self.advance(); // consume backslash
                if let TokenKind::Text(name) = self.peek_kind()
                    && name == "item"
                {
                    self.advance(); // consume "item"
                    if let Some(item) = self.parse_item()? {
                        items.push(item);
                    }
                    continue;
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

    /// Parse \item - handles two patterns:
    /// 1. \item{label}{content} - used in \arguments and \describe
    /// 2. \item content... or \item{label} content... - used in \itemize/\enumerate
    pub(super) fn parse_item(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();

        // Check for optional label {label}
        let label = if self.check(&TokenKind::OpenBrace) {
            Some(self.parse_braced_content()?)
        } else {
            None
        };

        // For \arguments pattern: \item{label}{content}
        // If we have a label and another brace follows, parse it as the content
        self.skip_whitespace();
        if label.is_some() && self.check(&TokenKind::OpenBrace) {
            let content = self.parse_braced_content()?;
            return Ok(Some(RdNode::Item { label, content }));
        }

        // Parse content until next \item or } (for \itemize/\enumerate)
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
                if next_pos < self.tokens.len()
                    && let TokenKind::Text(name) = &self.tokens[next_pos].kind
                    && name == "item"
                {
                    break;
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
    pub(super) fn parse_describe(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;

        let mut items = Vec::new();
        self.skip_whitespace_and_newlines();

        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            if self.check(&TokenKind::Backslash) {
                let pos = self.pos;
                self.advance();
                if let TokenKind::Text(name) = self.peek_kind()
                    && name == "item"
                {
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
                self.pos = pos;
            }
            self.advance();
        }

        self.expect(&TokenKind::CloseBrace)?;
        Ok(Some(RdNode::Describe(items)))
    }

    /// Parse \tabular{alignment}{content}
    pub(super) fn parse_tabular(&mut self) -> ParseResult<Option<RdNode>> {
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
                            self.consume_optional_empty_braces();
                            if !current_text.is_empty() {
                                current_cell.push(RdNode::Text(std::mem::take(&mut current_text)));
                            }
                            current_row.push(std::mem::take(&mut current_cell));
                        }
                        TokenKind::Text(name) if name == "cr" => {
                            self.advance();
                            self.consume_optional_empty_braces();
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

        // Flush remaining content; trailing whitespace after the last \cr
        // (e.g. a newline before the closing `}`) must not create a spurious empty row.
        if !current_text.trim_end().is_empty() {
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
    pub(super) fn parse_subsection(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        let title = self.parse_braced_content()?;

        self.skip_whitespace();
        let content = self.parse_braced_content()?;

        Ok(Some(RdNode::Subsection { title, content }))
    }

    /// Parse \href{url}{text}
    pub(super) fn parse_href(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let url = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        let text = self.parse_braced_content()?;

        Ok(Some(RdNode::Href { url, text }))
    }

    /// Parse \enc{encoded}{fallback}
    /// Preserves both arguments in AST for format-specific output selection
    pub(super) fn parse_enc(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let encoded = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        // Parse the fallback argument
        self.skip_whitespace();
        let fallback = if self.check(&TokenKind::OpenBrace) {
            self.expect(&TokenKind::OpenBrace)?;
            let fb = self.parse_text_until_close_brace()?;
            self.expect(&TokenKind::CloseBrace)?;
            fb
        } else {
            // If no fallback provided, use encoded as fallback
            encoded.clone()
        };

        Ok(Some(RdNode::Enc { encoded, fallback }))
    }

    /// Parse \link[pkg]{topic}, \link[pkg:bar]{text}, or \link[=dest]{text}
    pub(super) fn parse_link(&mut self, opt_arg: Option<String>) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        let content = self.parse_braced_content()?;

        let (package, topic, text) = if let Some(opt) = opt_arg {
            if let Some(dest) = opt.strip_prefix('=') {
                // \link[=dest]{text} form
                (None, dest.to_string(), Some(content))
            } else if let Some((pkg, topic_part)) = opt.split_once(':') {
                // \link[pkg:bar]{text} form - content is display text
                (Some(pkg.to_string()), topic_part.to_string(), Some(content))
            } else {
                // \link[pkg]{topic} form
                let topic = Self::extract_text_from_nodes(&content);
                (Some(opt), topic, None)
            }
        } else {
            // \link{topic} form
            let topic = Self::extract_text_from_nodes(&content);
            (None, topic, None)
        };

        Ok(Some(RdNode::Link {
            package,
            topic,
            text,
        }))
    }

    fn extract_text_from_nodes(nodes: &[RdNode]) -> String {
        nodes
            .first()
            .map(|n| match n {
                RdNode::Text(s) => s.clone(),
                _ => String::new(),
            })
            .unwrap_or_default()
    }

    /// Parse \eqn{latex}{ascii} or \deqn{latex}{ascii}
    pub(super) fn parse_equation(&mut self, display: bool) -> ParseResult<Option<RdNode>> {
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
    pub(super) fn parse_sexpr(&mut self, opt_arg: Option<String>) -> ParseResult<Option<RdNode>> {
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
    pub(super) fn parse_if(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let format = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        let content = self.parse_braced_content()?;

        Ok(Some(RdNode::If { format, content }))
    }

    /// Parse \ifelse{format}{then}{else}
    pub(super) fn parse_ifelse(&mut self) -> ParseResult<Option<RdNode>> {
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
    pub(super) fn parse_method(&mut self) -> ParseResult<Option<RdNode>> {
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
    pub(super) fn parse_s4method(&mut self) -> ParseResult<Option<RdNode>> {
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

    /// Parse \S3method{generic}{class} - equivalent to \method
    pub(super) fn parse_s3method(&mut self) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let generic = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let class = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        Ok(Some(RdNode::S3Method { generic, class }))
    }

    /// Parse \linkS4class[pkg]{classname} - link to S4 class documentation
    pub(super) fn parse_link_s4class(
        &mut self,
        opt_arg: Option<String>,
    ) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let classname = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        Ok(Some(RdNode::LinkS4Class {
            package: opt_arg,
            classname,
        }))
    }

    /// Parse \figure{file}{options}
    ///
    /// The \figure tag has three forms per "Writing R Extensions":
    /// 1. `\figure{filename}` - No options
    /// 2. `\figure{filename}{alternate text}` - Simple form
    /// 3. `\figure{filename}{options: string}` - Expert form
    ///
    /// Reference: https://cran.r-project.org/doc/manuals/r-devel/R-exts.html#Figures
    pub(super) fn parse_figure(&mut self, opt_arg: Option<String>) -> ParseResult<Option<RdNode>> {
        self.skip_whitespace();
        self.expect(&TokenKind::OpenBrace)?;
        let file = self.parse_text_until_close_brace()?;
        self.expect(&TokenKind::CloseBrace)?;

        // Check for optional second brace argument (options)
        self.skip_whitespace();
        let raw_options = if self.check(&TokenKind::OpenBrace) {
            self.advance(); // consume {
            let opts = self.parse_text_until_close_brace()?;
            self.expect(&TokenKind::CloseBrace)?;
            Some(opts)
        } else {
            opt_arg // Fallback to bracket arg if provided
        };

        // Parse options into structured form
        let options = raw_options.map(|opts| Self::parse_figure_options(&opts));

        Ok(Some(RdNode::Figure { file, options }))
    }

    /// Parse figure options string into structured form
    ///
    /// Expert form: starts with "options:" followed by at least one whitespace
    /// Simple form: everything else (the entire string is alternate text)
    fn parse_figure_options(opts: &str) -> FigureOptions {
        // Check for expert form: "options:" followed by whitespace
        if let Some(rest) = opts.strip_prefix("options:") {
            // Must have at least one whitespace after "options:"
            if rest.starts_with(char::is_whitespace) {
                return FigureOptions::ExpertOptions(rest.trim_start().to_string());
            }
        }
        // Simple form: entire string is alt text
        FigureOptions::AltText(opts.to_string())
    }
}
