//! Context-specific recovery-first flattening of canonical text leaves.

use std::fmt;

/// A leaf whose kind does not match the context requested by a flattener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LeafShapeError {
    mismatches: Vec<LeafShapeMismatch>,
    recovered: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeafKind {
    Text,
    RCode,
    Verb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LeafShapeMismatch {
    expected: LeafKind,
    found: LeafKind,
}

impl fmt::Display for LeafShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} leaf shape mismatch(es)", self.mismatches.len())
    }
}

impl std::error::Error for LeafShapeError {}

impl LeafShapeError {
    /// Text recovered while reporting this recoverable shape mismatch.
    pub(crate) fn recovered_text(&self) -> &str {
        &self.recovered
    }

    #[cfg(test)]
    pub(crate) fn mismatch_count(&self) -> usize {
        self.mismatches.len()
    }
}

fn flatten<F>(
    nodes: &[rd_ast::RdNode],
    expected: Option<LeafKind>,
    accept: F,
) -> Result<String, LeafShapeError>
where
    F: Fn(LeafKind) -> bool + Copy,
{
    let mut output = String::new();
    let mut mismatches = Vec::new();
    walk(nodes, expected, accept, &mut output, &mut mismatches);
    if mismatches.is_empty() {
        Ok(output)
    } else {
        Err(LeafShapeError {
            mismatches,
            recovered: output,
        })
    }
}

fn walk<F>(
    nodes: &[rd_ast::RdNode],
    expected: Option<LeafKind>,
    accept: F,
    output: &mut String,
    mismatches: &mut Vec<LeafShapeMismatch>,
) where
    F: Fn(LeafKind) -> bool + Copy,
{
    for node in nodes {
        match node {
            rd_ast::RdNode::Text(value) => {
                visit_leaf(LeafKind::Text, value, expected, accept, output, mismatches)
            }
            rd_ast::RdNode::RCode(value) => {
                visit_leaf(LeafKind::RCode, value, expected, accept, output, mismatches)
            }
            rd_ast::RdNode::Verb(value) => {
                visit_leaf(LeafKind::Verb, value, expected, accept, output, mismatches)
            }
            rd_ast::RdNode::Comment(_) => {}
            rd_ast::RdNode::Group(group) => {
                walk(group.children(), expected, accept, output, mismatches)
            }
            // Tagged nodes carry semantics that their caller must interpret.
            rd_ast::RdNode::Tagged(_) => {}
            // Raw children are the explicit recovery fallback.
            rd_ast::RdNode::Raw(raw) => walk(raw.children(), expected, accept, output, mismatches),
            _ => {}
        }
    }
}

fn visit_leaf<F>(
    found: LeafKind,
    value: &str,
    expected: Option<LeafKind>,
    accept: F,
    output: &mut String,
    mismatches: &mut Vec<LeafShapeMismatch>,
) where
    F: Fn(LeafKind) -> bool,
{
    if !accept(found) {
        mismatches.push(LeafShapeMismatch {
            expected: expected.expect("contextual flatteners specify an expected kind"),
            found,
        });
    }
    output.push_str(value);
}

/// Flatten prose leaves, accepting all canonical text-bearing leaf kinds.
#[cfg(test)]
pub(crate) fn flatten_prose_leaves(nodes: &[rd_ast::RdNode]) -> Result<String, LeafShapeError> {
    flatten(nodes, None, |_| true)
}

/// Flatten R code leaves, recovering by concatenating unexpected text or verbatim leaves.
#[cfg(test)]
pub(crate) fn flatten_rcode_leaves(nodes: &[rd_ast::RdNode]) -> Result<String, LeafShapeError> {
    flatten(nodes, Some(LeafKind::RCode), |kind| kind == LeafKind::RCode)
}

/// Flatten verbatim leaves, recovering by concatenating unexpected text or R code leaves.
pub(crate) fn flatten_verbatim_leaves(nodes: &[rd_ast::RdNode]) -> Result<String, LeafShapeError> {
    flatten(nodes, Some(LeafKind::Verb), |kind| kind == LeafKind::Verb)
}

#[cfg(test)]
mod tests {
    use super::{flatten_prose_leaves, flatten_rcode_leaves};
    use rd_ast::RdNode;

    #[test]
    fn prose_flattens_mixed_leaves_and_skips_comments() {
        let nodes = vec![
            RdNode::Text("before".into()),
            RdNode::Comment("% hidden".into()),
            RdNode::Text("\n".into()),
            RdNode::RCode("f(x)".into()),
            RdNode::Verb("literal".into()),
        ];
        assert_eq!(flatten_prose_leaves(&nodes).unwrap(), "before\nf(x)literal");
    }

    #[test]
    fn rcode_shape_mismatch_recovers_concatenated_text() {
        let nodes = vec![RdNode::Text("text".into()), RdNode::RCode("code".into())];
        let error = flatten_rcode_leaves(&nodes).unwrap_err();
        assert_eq!(error.recovered_text(), "textcode");
        assert_eq!(error.mismatch_count(), 1);
    }
}
