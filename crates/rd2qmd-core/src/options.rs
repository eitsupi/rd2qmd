use std::collections::HashMap;

/// Format for the Arguments section output.
///
/// This enum is non-exhaustive: downstream matches must include a wildcard,
/// even when every currently enabled format is handled. This keeps matches
/// valid when another dependency enables the `grid-table` feature.
///
/// ```
/// use rd2qmd_core::ArgumentsFormat;
/// let format = ArgumentsFormat::default();
/// let is_list_table = match format {
///     ArgumentsFormat::ListTable => true,
///     _ => false,
/// };
/// assert!(is_list_table);
/// ```
///
/// Omitting the wildcard is rejected with or without grid support:
///
/// ```compile_fail,E0004
/// use rd2qmd_core::ArgumentsFormat;
/// match ArgumentsFormat::default() {
///     ArgumentsFormat::PipeTable => {},
///     ArgumentsFormat::ListTable => {},
///     ArgumentsFormat::List => {},
///     #[cfg(feature = "grid-table")]
///     ArgumentsFormat::GridTable => {},
/// }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ArgumentsFormat {
    PipeTable,
    /// Pandoc grid table; requires the `grid-table` Cargo feature.
    #[cfg(feature = "grid-table")]
    GridTable,
    #[default]
    ListTable,
    List,
}

/// Format for Rd description lists (`\describe{}`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DescribeFormat {
    /// Pandoc definition list, requiring a definition-list extension.
    #[default]
    DefinitionList,
    /// CommonMark-compatible bullet list with emphasized terms.
    List,
    /// Terms become headings below their enclosing section; beyond H6, use lists.
    /// Arguments pipe-table cells also fall back to lists, flattened with `<br>`.
    Headings,
}

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
    pub arguments_format: ArgumentsFormat,
    pub describe_format: DescribeFormat,
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
            arguments_format: ArgumentsFormat::default(),
            describe_format: DescribeFormat::default(),
            include_html_output: false,
            prefer_ascii_math: false,
        }
    }
}
