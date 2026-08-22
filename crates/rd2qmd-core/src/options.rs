use std::collections::HashMap;

pub use rd2qmd_mdast::ArgumentsFormat;

/// Options for converting an Rd document to mdast.
#[derive(Debug, Clone)]
pub struct RdToMdastOptions {
    pub include_title_heading: bool,
    pub internal_link_url: Option<String>,
    pub alias_map: Option<HashMap<String, String>>,
    pub unqualified_link_url: Option<String>,
    pub package_urls: Option<HashMap<String, String>>,
    pub external_link_url: Option<String>,
    pub exec_dontrun: bool,
    pub exec_donttest: bool,
    pub quarto_code_blocks: bool,
    pub include_html_output: bool,
    pub prefer_ascii_math: bool,
}

impl Default for RdToMdastOptions {
    fn default() -> Self {
        Self {
            include_title_heading: true,
            internal_link_url: None,
            alias_map: None,
            unqualified_link_url: None,
            package_urls: None,
            external_link_url: None,
            exec_dontrun: false,
            exec_donttest: true,
            quarto_code_blocks: true,
            include_html_output: false,
            prefer_ascii_math: false,
        }
    }
}
