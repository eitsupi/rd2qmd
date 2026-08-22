//! Raw-HTML handling for the Typst writer.
//!
//! Rd's `\out{}` (and `\if{html}{}`, when HTML output is enabled) yields a
//! fragment of literal HTML. Typst has no way to splice an unparsed HTML
//! string into a document, but since 0.15 it can *build* HTML elements with
//! `html.elem` when compiling for the HTML export target.
//!
//! So a fragment that is one simple element is re-expressed as an
//! `html.elem` call, wrapped in a `target()` check so that compiling the
//! same file to PDF neither fails nor emits markup. Anything more involved
//! than a single element (nested tags, multiple siblings, comments) is kept
//! verbatim as raw text rather than dropped, so no content goes missing.

use super::{TypstWriter, typst_string};

impl TypstWriter<'_> {
    pub(super) fn write_html(&mut self, h: &crate::mdast::Html) {
        let value = h.value.trim();
        if value.is_empty() {
            return;
        }
        match parse_simple_element(value) {
            Some(element) => {
                self.output.push_str(&render_html_elem(&element));
            }
            None if !value.contains(['<', '>']) => {
                let decoded = html_escape::decode_html_entities(value);
                self.output.push_str(&super::escape_text(&decoded));
            }
            None => {
                self.output
                    .push_str(&format!("#raw({})", typst_string(value)));
            }
        }
        self.at_line_start = false;
    }
}

/// One HTML element with no nested elements in its body.
struct SimpleElement {
    tag: String,
    attributes: Vec<(String, String)>,
    body: Option<String>,
}

/// Emit `html.elem`, guarded so the same document still compiles to PDF.
fn render_html_elem(element: &SimpleElement) -> String {
    let mut call = format!("html.elem({}", typst_string(&element.tag));
    if !element.attributes.is_empty() {
        let attributes: Vec<_> = element
            .attributes
            .iter()
            .map(|(name, value)| format!("{}: {}", attribute_key(name), typst_string(value)))
            .collect();
        call.push_str(&format!(", attrs: ({})", attributes.join(", ")));
    }
    call.push(')');
    if let Some(body) = &element.body
        && !body.is_empty()
    {
        call.push_str(&format!("[{}]", super::escape_text(body)));
    }
    format!("#context {{ if target() == \"html\" {{ {call} }} }}")
}

/// Quote an attribute name that is not a bare Typst identifier
/// (`aria-label`, `data-x`, ...).
fn attribute_key(name: &str) -> String {
    let bare = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if bare {
        name.to_owned()
    } else {
        typst_string(name)
    }
}

/// Parse `<tag attr="value">text</tag>`, `<tag/>`, or `<tag>`.
///
/// Deliberately narrow: it recognizes exactly the shapes that appear in
/// practice in `\out{}` (`<br>`, `<sup>2</sup>`, `<span class="x">text</span>`)
/// and gives up on anything else rather than half-parsing HTML.
fn parse_simple_element(value: &str) -> Option<SimpleElement> {
    let rest = value.strip_prefix('<')?;
    if rest.starts_with('/') || rest.starts_with('!') {
        return None;
    }
    let (open_tag, rest) = rest.split_once('>')?;
    let open_tag = open_tag.trim();
    let (open_tag, self_closing) = match open_tag.strip_suffix('/') {
        Some(stripped) => (stripped.trim_end(), true),
        None => (open_tag, false),
    };

    let (tag, attribute_text) = match open_tag.find(char::is_whitespace) {
        Some(index) => (&open_tag[..index], open_tag[index..].trim()),
        None => (open_tag, ""),
    };
    if tag.is_empty() || !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let attributes = parse_attributes(attribute_text)?;

    if self_closing || rest.is_empty() {
        return Some(SimpleElement {
            tag: tag.to_owned(),
            attributes,
            body: None,
        });
    }

    // A body containing further markup is out of scope.
    let closing = format!("</{tag}>");
    let body = rest.strip_suffix(&closing)?;
    if body.contains('<') || body.contains('>') {
        return None;
    }

    Some(SimpleElement {
        tag: tag.to_owned(),
        attributes,
        body: Some(html_escape::decode_html_entities(body).into_owned()),
    })
}

/// Parse `name="value"` pairs. Unquoted or valueless attributes are not
/// recognized, which makes the whole fragment fall back to raw text.
fn parse_attributes(text: &str) -> Option<Vec<(String, String)>> {
    let mut attributes = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let (name, tail) = rest.split_once('=')?;
        let name = name.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            return None;
        }
        let tail = tail.trim_start();
        let quote = tail.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let (value, tail) = tail[1..].split_once(quote)?;
        attributes.push((
            name.to_owned(),
            html_escape::decode_html_entities(value).into_owned(),
        ));
        rest = tail.trim_start();
    }
    Some(attributes)
}
