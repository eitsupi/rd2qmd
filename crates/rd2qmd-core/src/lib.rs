//! rd2qmd-core: Core library for converting Rd files to Quarto Markdown
//!
//! This crate provides:
//! - Rd file parsing (lexer + recursive descent parser)
//! - Rd AST representation
//! - Rd AST to mdast conversion
//! - mdast to Quarto Markdown output

pub fn hello() -> &'static str {
    "rd2qmd-core"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello() {
        assert_eq!(hello(), "rd2qmd-core");
    }
}
