//! Emphasis inside a word must be written with `*`, because CommonMark does
//! not recognize `_` as an emphasis delimiter there.
use rd2qmd_core::{ArgumentsFormat, RdConvertOptions};

const INPUT: &str = r"\name{intra}\title{Intra}
\description{before\emph{em}after and \emph{plain} and \strong{\emph{s}}.}
\arguments{\item{x}{before\emph{em}after and \emph{plain}.}}";

fn convert(arguments_format: ArgumentsFormat) -> String {
    let parsed = rd2qmd_source::parse(INPUT).unwrap();
    rd2qmd_core::convert_rd_document(
        parsed.document(),
        &RdConvertOptions {
            arguments_format,
            ..Default::default()
        },
    )
}

#[test]
fn intraword_emphasis_uses_star_in_body_and_arguments() {
    for format in [
        ArgumentsFormat::ListTable,
        ArgumentsFormat::List,
        ArgumentsFormat::PipeTable,
        ArgumentsFormat::GridTable,
    ] {
        let out = convert(format.clone());
        assert_eq!(
            out.matches("before*em*after").count(),
            2,
            "{format:?}:\n{out}"
        );
        assert!(!out.contains("before_em_after"), "{format:?}:\n{out}");
        assert!(out.contains("_plain_"), "{format:?}:\n{out}");
        assert!(out.contains("**_s_**"), "{format:?}:\n{out}");
    }
}
