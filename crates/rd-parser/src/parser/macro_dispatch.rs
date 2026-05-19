use crate::ast::{RdNode, SpecialChar};
use crate::lexer::{Token, TokenKind};
use super::{ParseResult, Parser};

impl Parser {
    /// Parse a macro (after seeing backslash)
    pub(super) fn parse_macro(&mut self) -> ParseResult<Option<RdNode>> {
        self.expect(&TokenKind::Backslash)?;

        let name = self.parse_macro_name()?;

        // Handle special characters (no braces needed).
        // Consume an immediately following empty `{}` terminator if present —
        // a common Rd idiom to unambiguously end a zero-arg macro (e.g. `\dots{}`).
        match name.as_str() {
            "R" => {
                self.consume_optional_empty_braces();
                return Ok(Some(RdNode::Special(SpecialChar::R)));
            }
            "dots" | "ldots" => {
                self.consume_optional_empty_braces();
                return Ok(Some(RdNode::Special(SpecialChar::Dots)));
            }
            "cr" => {
                self.consume_optional_empty_braces();
                return Ok(Some(RdNode::LineBreak));
            }
            "tab" => {
                self.consume_optional_empty_braces();
                return Ok(Some(RdNode::Tab));
            }
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
            "abbr" => self.parse_text_arg().map(|s| Some(RdNode::Abbr(s))),
            "cite" => self.parse_text_arg().map(|s| Some(RdNode::Cite(s))),

            // Link-like elements
            "href" => self.parse_href(),
            "link" => self.parse_link(opt_arg),
            "linkS4class" => self.parse_link_s4class(opt_arg),
            "Sexpr" => self.parse_sexpr(opt_arg),

            // DOI link
            "doi" => self.parse_text_arg().map(|s| Some(RdNode::Doi(s))),

            // Equations
            "eqn" => self.parse_equation(false),
            "deqn" => self.parse_equation(true),

            // Conditionals
            "if" => self.parse_if(),
            "ifelse" => self.parse_ifelse(),
            "out" => self.parse_verbatim_inline().map(|s| Some(RdNode::Out(s))),

            // Encoding tag - use first (UTF-8) argument, ignore ASCII fallback
            "enc" => self.parse_enc(),

            // Method declarations (in \usage)
            "method" => self.parse_method(),
            "S4method" => self.parse_s4method(),
            "S3method" => self.parse_s3method(),

            // Item (in lists)
            "item" => self.parse_item(),

            // Figure
            "figure" => self.parse_figure(opt_arg),

            // Example control macros
            "dontrun" => self.parse_inline_nodes().map(|n| Some(RdNode::DontRun(n))),
            "donttest" => self.parse_inline_nodes().map(|n| Some(RdNode::DontTest(n))),
            "dontshow" | "testonly" => {
                self.parse_inline_nodes().map(|n| Some(RdNode::DontShow(n)))
            }
            "dontdiff" => self.parse_inline_nodes().map(|n| Some(RdNode::DontDiff(n))),

            // Unknown macro - store generically
            _ => self.parse_generic_macro(&name),
        }
    }

    /// Parse macro name (text following backslash)
    ///
    /// Macro names consist only of ASCII alphanumeric characters per parseRd.pdf.
    /// If the text token has a non-alphanumeric suffix (e.g. `dots)` after `\`),
    /// only the alphanumeric prefix is consumed as the name and the remainder is
    /// reinserted into the token stream so it can be parsed as text content.
    pub(super) fn parse_macro_name(&mut self) -> ParseResult<String> {
        match self.peek_kind() {
            TokenKind::Text(s) => {
                let alpha_end = s
                    .find(|c: char| !c.is_ascii_alphanumeric())
                    .unwrap_or(s.len());
                if alpha_end == 0 {
                    return Ok(String::new());
                }
                let name = s[..alpha_end].to_string();
                let rest = s[alpha_end..].to_string();
                let span = self.peek().map(|t| t.span).unwrap_or_default();
                self.advance();
                if !rest.is_empty() {
                    self.tokens.insert(
                        self.pos,
                        Token {
                            kind: TokenKind::Text(rest),
                            span,
                        },
                    );
                }
                Ok(name)
            }
            // Special single-character escapes
            _ => Ok(String::new()),
        }
    }

    /// Parse unknown macro generically
    pub(super) fn parse_generic_macro(&mut self, name: &str) -> ParseResult<Option<RdNode>> {
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
}
