//! Bare brace groups in Rd prose render their content in place, as R does,
//! while braces inside code-like macros stay literal.
use rd2qmd_core::{DescribeFormat, RdConvertOptions};

fn convert(input: &str, describe_format: DescribeFormat) -> String {
    let parsed = rd2qmd_source::parse(input).unwrap();
    rd2qmd_core::convert_rd_document(
        parsed.document(),
        &RdConvertOptions {
            describe_format,
            ..Default::default()
        },
    )
}

fn convert_default(input: &str) -> String {
    convert(input, DescribeFormat::default())
}

#[test]
fn group_in_details_text() {
    let out = convert_default("\\name{t}\\title{T}\\details{\n{ a, b } bare group\n}");
    assert!(out.contains("a, b bare group"), "{out}");
    assert!(!out.contains('{'), "{out}");
}

#[test]
fn nested_group() {
    let out = convert_default("\\name{t}\\title{T}\\details{{a {b} c} end}");
    assert!(out.contains("a b c end"), "{out}");
}

#[test]
fn group_with_markup() {
    let out = convert_default("\\name{t}\\title{T}\\details{{\\emph{x} y} z}");
    assert!(out.contains("_x_ y z"), "{out}");
}

#[test]
fn group_in_arguments_item() {
    let out = convert_default("\\name{t}\\title{T}\\arguments{\\item{x}{{ a, b } arg}}");
    assert!(out.contains("a, b arg"), "{out}");
    assert!(!out.contains("{ a"), "{out}");
}

#[test]
fn group_in_describe_item_and_label() {
    let input = "\\name{t}\\title{T}\\details{\\describe{\\item{{lab}}{{ body } text}}}";
    for format in [
        DescribeFormat::PandocDefinitionList,
        DescribeFormat::List,
        DescribeFormat::Headings,
    ] {
        let out = convert(input, format);
        assert!(out.contains("lab"), "{out}");
        assert!(out.contains("body text"), "{out}");
        assert!(!out.contains('{'), "{out}");
    }
}

#[test]
fn group_in_itemize_and_table_cell() {
    let out = convert_default(
        "\\name{t}\\title{T}\\details{\\itemize{\\item {one} item}\\tabular{ll}{{p} \\tab {q} \\cr}}",
    );
    assert!(out.contains("one item"), "{out}");
    assert!(out.contains("p"), "{out}");
    assert!(out.contains("q"), "{out}");
    assert!(!out.contains('{'), "{out}");
}

#[test]
fn group_in_title() {
    let out = convert_default("\\name{t}\\title{T {grp} title}");
    assert!(out.contains("# T grp title"), "{out}");
}

#[test]
fn braces_in_code_stay_literal() {
    let out = convert_default(
        "\\name{t}\\title{T}\\details{\\code{{z}} and \\verb{{v}}\n\\preformatted{{p}}}",
    );
    assert!(out.contains("`{z}`"), "{out}");
    assert!(out.contains("`{v}`"), "{out}");
    assert!(out.contains("{p}"), "{out}");
}
