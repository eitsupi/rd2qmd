//! Foundational traversal helpers for the rd_ast conversion migration.
//!
//! This module coexists with the legacy convert implementation while conversion
//! is migrated in small, independently testable steps.

mod leaf_text;
mod traversal;

#[allow(unused_imports)]
pub(crate) use leaf_text::{
    LeafShapeError, flatten_prose_leaves, flatten_rcode_leaves, flatten_verbatim_leaves,
};
#[allow(unused_imports)]
pub(crate) use traversal::scan_paragraphs;
