//! Conversion of R-like Usage and Examples sections from rd_ast nodes.

use rd_ast::{RdExampleControlKind, RdMethodKind, RdNode, RdPath};
use rd2qmd_mdast::Node;

use super::leaf_text::flatten_verbatim_leaves;

/// Execution and output-format policy for an Examples section.
pub(crate) struct ExampleOptions {
    pub(crate) exec_dontrun: bool,
    pub(crate) exec_donttest: bool,
    pub(crate) quarto_code_blocks: bool,
}

/// Flatten a Usage section while preserving its source whitespace.
pub(crate) fn convert_usage(nodes: &[RdNode]) -> String {
    let mut output = String::new();
    append_rcode(nodes, &mut output);
    output
}

/// Convert an Examples section into separately fenced code blocks.
pub(crate) fn convert_examples(nodes: &[RdNode], options: &ExampleOptions) -> Vec<Node> {
    let mut result = Vec::new();
    let mut current_code = String::new();
    let mut has_executable = false;
    let base_path = RdPath::new(Vec::new());

    for node in nodes {
        let Some(control) = node.example_control(&base_path) else {
            has_executable = true;
            append_rcode(std::slice::from_ref(node), &mut current_code);
            continue;
        };

        match control.kind() {
            RdExampleControlKind::DontRun => {
                flush_code(&mut current_code, &mut result, true);
                has_executable = false;

                let code = recover_verbatim(control.body());
                push_code(&mut result, &code, options.exec_dontrun);
            }
            RdExampleControlKind::DontTest => {
                flush_code(&mut current_code, &mut result, true);
                has_executable = false;

                let mut code = String::new();
                append_rcode(control.body(), &mut code);
                push_code(&mut result, &code, options.exec_donttest);
            }
            RdExampleControlKind::DontShow | RdExampleControlKind::TestOnly => {
                let mut code = String::new();
                append_rcode(control.body(), &mut code);
                let trimmed = code.trim();
                let open_braces = trimmed
                    .chars()
                    .filter(|&character| character == '{')
                    .count();
                let close_braces = trimmed
                    .chars()
                    .filter(|&character| character == '}')
                    .count();

                // roxygen2's @examplesIf emits an incomplete opening wrapper and a
                // separate leading-`}` closing wrapper. Neither fragment is code.
                let is_start_wrapper = open_braces > close_braces;
                let is_end_wrapper = trimmed.starts_with('}');

                if !is_start_wrapper && !is_end_wrapper && !trimmed.is_empty() {
                    flush_code(&mut current_code, &mut result, true);
                    has_executable = false;

                    if options.quarto_code_blocks {
                        push_code(&mut result, &format!("#| include: false\n{trimmed}"), true);
                    }
                }
            }
            RdExampleControlKind::DontDiff => {
                has_executable = true;
                append_rcode(control.body(), &mut current_code);
            }
            _ => {
                has_executable = true;
                append_rcode(control.body(), &mut current_code);
            }
        }
    }

    if has_executable {
        flush_code(&mut current_code, &mut result, true);
    } else if !current_code.trim().is_empty() {
        flush_code(&mut current_code, &mut result, false);
    }

    result
}

fn recover_verbatim(nodes: &[RdNode]) -> String {
    match flatten_verbatim_leaves(nodes) {
        Ok(code) => code,
        Err(error) => error.recovered_text().to_owned(),
    }
}

fn push_code(result: &mut Vec<Node>, code: &str, executable: bool) {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return;
    }

    if executable {
        result.push(Node::code_with_meta(
            Some("r".to_string()),
            Some("executable".to_string()),
            trimmed,
        ));
    } else {
        result.push(Node::code(Some("r".to_string()), trimmed));
    }
}

fn flush_code(code: &mut String, result: &mut Vec<Node>, executable: bool) {
    push_code(result, code, executable);
    code.clear();
}

fn append_rcode(nodes: &[RdNode], output: &mut String) {
    let mut index = 0;
    let base_path = RdPath::new(Vec::new());

    while index < nodes.len() {
        match &nodes[index] {
            RdNode::RCode(code) => output.push_str(code),
            RdNode::Comment(_) => {}
            RdNode::Group(group) => append_rcode(group.children(), output),
            RdNode::Raw(raw) => append_rcode(raw.children(), output),
            node => {
                if let Some(symbol) = node.text_symbol(&base_path) {
                    output.push_str(symbol.fallback_text());
                    index += 1;
                    continue;
                }

                let Some(method) = node.method(&base_path) else {
                    index += 1;
                    continue;
                };

                match method.kind() {
                    RdMethodKind::Method | RdMethodKind::S3Method => {
                        if method.qualifier() == "default" {
                            output.push_str("# Default S3 method\n");
                        } else {
                            output.push_str(&format!(
                                "# S3 method for class '{}'\n",
                                method.qualifier()
                            ));
                        }
                    }
                    RdMethodKind::S4Method => output.push_str(&format!(
                        "# S4 method for signature '{}'\n",
                        method.qualifier()
                    )),
                    _ => {}
                }

                if is_infix_operator(method.generic())
                    && let Some(RdNode::RCode(call_text)) = nodes.get(index + 1)
                    && let Some(formatted) = try_format_infix_call(method.generic(), call_text)
                {
                    output.push_str(&formatted);
                    index += 2;
                    continue;
                }

                output.push_str(method.generic());
            }
        }
        index += 1;
    }
}

fn try_format_infix_call(generic: &str, call_text: &str) -> Option<String> {
    let leading_len = call_text.len() - call_text.trim_start_matches([' ', '\t']).len();
    let call_text_trimmed = &call_text[leading_len..];
    if !call_text_trimmed.starts_with('(') {
        return None;
    }

    let search_end = call_text_trimmed
        .find('\n')
        .unwrap_or(call_text_trimmed.len());
    let paren_end = find_matching_paren(&call_text_trimmed[..search_end])?;
    let args_content = &call_text_trimmed[1..paren_end];
    let trailing = &call_text[leading_len + paren_end + 1..];
    let args = parse_function_args(args_content);
    let formatted = format_infix_call(generic, &args)?;

    Some(format!("{formatted}{trailing}"))
}

/// Check if a generic name is an infix operator.
fn is_infix_operator(name: &str) -> bool {
    // Binary infix operators (with spaces)
    const PADDED_OPS: &[&str] = &[
        "+", "-", "*", "/", "==", "!=", "<", ">", "<=", ">=", "&", "|",
    ];
    // Infix operators without spaces
    const UNPADDED_OPS: &[&str] = &["^", "[", "[[", "$", ":", "::", ":::"];

    // User-defined infix operators: %...% (includes %% with length 2)
    if name.starts_with('%') && name.ends_with('%') && name.len() >= 2 {
        return true;
    }

    PADDED_OPS.contains(&name) || UNPADDED_OPS.contains(&name)
}

/// Check if operator should have spaces around it.
fn is_padded_infix(name: &str) -> bool {
    const PADDED_OPS: &[&str] = &[
        "+", "-", "*", "/", "==", "!=", "<", ">", "<=", ">=", "&", "|",
    ];

    // User-defined infix operators also get spaces (includes %% with length 2)
    if name.starts_with('%') && name.ends_with('%') && name.len() >= 2 {
        return true;
    }

    PADDED_OPS.contains(&name)
}

/// Find the index of the matching closing parenthesis.
fn find_matching_paren(s: &str) -> Option<usize> {
    if !s.starts_with('(') {
        return None;
    }

    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse function arguments, respecting nested parentheses.
fn parse_function_args(args_content: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut depth = 0;

    for c in args_content.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                current_arg.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current_arg.push(c);
            }
            ',' if depth == 0 => {
                args.push(current_arg.trim().to_string());
                current_arg = String::new();
            }
            _ => {
                current_arg.push(c);
            }
        }
    }

    // Push the last argument
    let last = current_arg.trim().to_string();
    if !last.is_empty() {
        args.push(last);
    }

    args
}

/// Format an infix operator call in natural form.
fn format_infix_call(operator: &str, args: &[String]) -> Option<String> {
    match operator {
        // Subscript operators
        "[" => {
            // x[i] or x[i, j, ...]
            if args.is_empty() {
                return None;
            }
            let obj = &args[0];
            let indices = &args[1..];
            if indices.is_empty() {
                Some(format!("{}[]", obj))
            } else {
                Some(format!("{}[{}]", obj, indices.join(", ")))
            }
        }
        "[[" => {
            // x[[i]] or x[[i, j]]
            if args.is_empty() {
                return None;
            }
            let obj = &args[0];
            let indices = &args[1..];
            if indices.is_empty() {
                Some(format!("{}[[]]", obj))
            } else {
                Some(format!("{}[[{}]]", obj, indices.join(", ")))
            }
        }
        "$" => {
            // x$name
            if args.len() != 2 {
                return None;
            }
            Some(format!("{}${}", args[0], args[1]))
        }
        // Namespace operators
        "::" | ":::" => {
            if args.len() != 2 {
                return None;
            }
            Some(format!("{}{}{}", args[0], operator, args[1]))
        }
        // Binary operators (padded and unpadded)
        _ => {
            // For binary operators, we need exactly 2 arguments
            if args.len() != 2 {
                return None;
            }
            if is_padded_infix(operator) {
                Some(format!("{} {} {}", args[0], operator, args[1]))
            } else {
                // Unpadded (like ^)
                Some(format!("{}{}{}", args[0], operator, args[1]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rd_ast::{RdNode, RdTag, producer};
    use rd2qmd_mdast::Node;

    use super::{ExampleOptions, convert_examples, convert_usage};

    fn example_options(
        exec_dontrun: bool,
        exec_donttest: bool,
        quarto_code_blocks: bool,
    ) -> ExampleOptions {
        ExampleOptions {
            exec_dontrun,
            exec_donttest,
            quarto_code_blocks,
        }
    }

    fn control(tag: RdTag, body: Vec<RdNode>) -> RdNode {
        RdNode::tagged(tag, None, body)
    }

    fn executable(value: &str) -> Node {
        Node::code_with_meta(Some("r".to_string()), Some("executable".to_string()), value)
    }

    fn plain(value: &str) -> Node {
        Node::code(Some("r".to_string()), value)
    }

    fn method(tag: RdTag, generic: &str, qualifier: &str) -> RdNode {
        RdNode::tagged(
            tag,
            None,
            vec![
                RdNode::group(vec![RdNode::Text(generic.to_owned())]),
                RdNode::group(vec![RdNode::Text(qualifier.to_owned())]),
            ],
        )
    }

    fn raw(nodes: Vec<RdNode>) -> RdNode {
        RdNode::Raw(producer::raw_node(None, None, nodes, None, vec![]))
    }

    #[test]
    fn converts_multi_signature_usage_with_method_headers_and_infix_calls() {
        let nodes = vec![
            RdNode::RCode("\n".to_owned()),
            method(RdTag::Method, "+", "default"),
            RdNode::RCode("(e1, e2)\n".to_owned()),
            method(RdTag::S3Method, "print", "widget"),
            RdNode::RCode("(x, ...)\n".to_owned()),
            method(RdTag::S4Method, "[", "Widget"),
            RdNode::RCode("(x, i, j)\n".to_owned()),
            method(RdTag::S3Method, "summary", "widget"),
            RdNode::RCode("(x)\n".to_owned()),
            RdNode::RCode("plain(y)\n".to_owned()),
        ];

        assert_eq!(
            convert_usage(&nodes),
            "\n# Default S3 method\n\
             e1 + e2\n\
             # S3 method for class 'widget'\n\
             print(x, ...)\n\
             # S4 method for signature 'Widget'\n\
             x[i, j]\n\
             # S3 method for class 'widget'\n\
             summary(x)\n\
             plain(y)\n"
        );
    }

    #[test]
    fn leaves_multiline_infix_call_unformatted() {
        let nodes = vec![
            method(RdTag::Method, "+", "numbers"),
            RdNode::RCode("(\n".to_owned()),
            RdNode::RCode("  e1,\n".to_owned()),
            RdNode::RCode("  e2\n".to_owned()),
            RdNode::RCode(")\n".to_owned()),
        ];

        assert_eq!(
            convert_usage(&nodes),
            "# S3 method for class 'numbers'\n+(\n  e1,\n  e2\n)\n"
        );
    }

    #[test]
    fn leaves_infix_call_starting_on_a_new_line_unformatted() {
        let same_line = vec![
            method(RdTag::Method, "+", "numbers"),
            RdNode::RCode("(e1, e2)\n".to_owned()),
        ];
        let next_line = vec![
            method(RdTag::Method, "+", "numbers"),
            RdNode::RCode("\n(e1, e2)\n".to_owned()),
        ];

        assert_eq!(
            convert_usage(&same_line),
            "# S3 method for class 'numbers'\ne1 + e2\n"
        );
        assert_eq!(
            convert_usage(&next_line),
            "# S3 method for class 'numbers'\n+\n(e1, e2)\n"
        );
    }

    #[test]
    fn preserves_text_symbols_in_usage_code() {
        let nodes = vec![
            RdNode::RCode("f(".to_owned()),
            RdNode::tagged(RdTag::Dots, None, vec![]),
            RdNode::RCode(")\n".to_owned()),
        ];

        assert_eq!(convert_usage(&nodes), "f(...)\n");
    }

    #[test]
    fn preserves_rcode_nested_in_group_and_raw_nodes() {
        let nodes = vec![
            RdNode::group(vec![RdNode::RCode("grouped()\n".to_owned())]),
            raw(vec![RdNode::RCode("recovered()\n".to_owned())]),
        ];

        assert_eq!(convert_usage(&nodes), "grouped()\nrecovered()\n");
    }

    #[test]
    fn drops_comments_without_affecting_surrounding_rcode() {
        let nodes = vec![
            RdNode::RCode("first()\n".to_owned()),
            RdNode::Comment("% hidden".to_owned()),
            RdNode::RCode("\nsecond()\n".to_owned()),
        ];

        assert_eq!(convert_usage(&nodes), "first()\n\nsecond()\n");
    }

    #[test]
    fn distinguishes_default_and_named_s3_method_headers() {
        let nodes = vec![
            method(RdTag::S3Method, "print", "default"),
            RdNode::RCode("(x)\n".to_owned()),
            method(RdTag::Method, "print", "widget"),
            RdNode::RCode("(x)\n".to_owned()),
        ];

        assert_eq!(
            convert_usage(&nodes),
            "# Default S3 method\nprint(x)\n\
             # S3 method for class 'widget'\nprint(x)\n"
        );
    }

    #[test]
    fn preserves_leading_and_trailing_whitespace() {
        let nodes = vec![
            RdNode::RCode("  leading\n".to_owned()),
            RdNode::RCode("trailing  \n\n".to_owned()),
        ];

        assert_eq!(convert_usage(&nodes), "  leading\ntrailing  \n\n");
    }

    #[test]
    fn converts_regular_examples_to_one_executable_block() {
        let nodes = vec![
            RdNode::RCode("\nfirst()\n".to_owned()),
            RdNode::group(vec![RdNode::RCode("second()\n".to_owned())]),
        ];

        assert_eq!(
            convert_examples(&nodes, &example_options(false, false, false)),
            vec![executable("first()\nsecond()")]
        );
    }

    #[test]
    fn dontrun_is_a_separate_block_with_configurable_execution() {
        let nodes = vec![
            RdNode::RCode("before()\n".to_owned()),
            control(RdTag::DontRun, vec![RdNode::Verb("never()\n".to_owned())]),
            RdNode::RCode("after()\n".to_owned()),
        ];

        assert_eq!(
            convert_examples(&nodes, &example_options(false, false, false)),
            vec![
                executable("before()"),
                plain("never()"),
                executable("after()")
            ]
        );
        assert_eq!(
            convert_examples(&nodes, &example_options(true, false, false)),
            vec![
                executable("before()"),
                executable("never()"),
                executable("after()")
            ]
        );
    }

    #[test]
    fn donttest_is_a_separate_block_with_configurable_execution() {
        let nodes = vec![
            RdNode::RCode("before()\n".to_owned()),
            control(RdTag::DontTest, vec![RdNode::RCode("slow()\n".to_owned())]),
            RdNode::RCode("after()\n".to_owned()),
        ];

        assert_eq!(
            convert_examples(&nodes, &example_options(false, false, false)),
            vec![
                executable("before()"),
                plain("slow()"),
                executable("after()")
            ]
        );
        assert_eq!(
            convert_examples(&nodes, &example_options(false, true, false)),
            vec![
                executable("before()"),
                executable("slow()"),
                executable("after()")
            ]
        );
    }

    #[test]
    fn genuine_dontshow_is_hidden_in_quarto_and_omitted_otherwise() {
        let nodes = vec![control(
            RdTag::DontShow,
            vec![RdNode::RCode("setup()\n".to_owned())],
        )];

        assert_eq!(
            convert_examples(&nodes, &example_options(false, false, true)),
            vec![executable("#| include: false\nsetup()")]
        );
        assert!(convert_examples(&nodes, &example_options(false, false, false)).is_empty());
    }

    #[test]
    fn examples_if_dontshow_wrapper_fragments_are_skipped() {
        let nodes = vec![
            control(
                RdTag::DontShow,
                vec![RdNode::RCode("if (condition) {\n".to_owned())],
            ),
            RdNode::RCode("inside()\n".to_owned()),
            control(
                RdTag::DontShow,
                vec![RdNode::RCode("} # end wrapper\n".to_owned())],
            ),
        ];

        assert_eq!(
            convert_examples(&nodes, &example_options(false, false, true)),
            vec![executable("inside()")]
        );
    }

    #[test]
    fn dontdiff_merges_with_surrounding_executable_code() {
        let nodes = vec![
            RdNode::RCode("before()\n".to_owned()),
            control(
                RdTag::DontDiff,
                vec![RdNode::RCode("variable_output()\n".to_owned())],
            ),
            RdNode::RCode("after()\n".to_owned()),
        ];

        assert_eq!(
            convert_examples(&nodes, &example_options(false, false, false)),
            vec![executable("before()\nvariable_output()\nafter()")]
        );
    }

    #[test]
    fn testonly_matches_dontshow_for_complete_code_and_wrappers() {
        let complete = vec![control(
            RdTag::TestOnly,
            vec![RdNode::RCode("stopifnot(TRUE)\n".to_owned())],
        )];
        let wrappers = vec![
            control(
                RdTag::TestOnly,
                vec![RdNode::RCode("if (condition) {\n".to_owned())],
            ),
            RdNode::RCode("inside()\n".to_owned()),
            control(RdTag::TestOnly, vec![RdNode::RCode("}\n".to_owned())]),
        ];

        assert_eq!(
            convert_examples(&complete, &example_options(false, false, true)),
            vec![executable("#| include: false\nstopifnot(TRUE)")]
        );
        assert!(convert_examples(&complete, &example_options(false, false, false)).is_empty());
        assert_eq!(
            convert_examples(&wrappers, &example_options(false, false, true)),
            vec![executable("inside()")]
        );
    }

    #[test]
    fn empty_trailing_control_does_not_create_a_spurious_block() {
        let nodes = vec![
            RdNode::RCode("before()\n".to_owned()),
            control(RdTag::DontRun, vec![]),
        ];

        assert_eq!(
            convert_examples(&nodes, &example_options(false, false, false)),
            vec![executable("before()")]
        );
    }
}
