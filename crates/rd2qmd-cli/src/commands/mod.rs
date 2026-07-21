//! Subcommand handlers, one module per `rd2qmd` subcommand.

mod convert;
mod index;
mod init;
mod parse;

pub(crate) use convert::run_convert_command;
pub(crate) use index::run_index_command;
pub(crate) use init::run_init_command;
pub(crate) use parse::run_parse_command;

use std::path::Path;

pub(crate) fn display_diagnostic(path: &Path, diagnostic: &rd2qmd_source::Diagnostic) {
    let start = diagnostic.span().start();
    eprintln!(
        "{}:{}:{}: {:?}[{:?}]: {}",
        path.display(),
        start.line(),
        start.column(),
        diagnostic.severity(),
        diagnostic.code(),
        diagnostic.message(),
    );
}
