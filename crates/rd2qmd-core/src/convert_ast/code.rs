//! Conversion of Usage-section code from rd_ast nodes.

use rd_ast::{RdMethodKind, RdNode, RdPath};

/// Flatten a Usage section while preserving its source whitespace.
pub(crate) fn convert_usage(nodes: &[RdNode]) -> String {
    let mut output = String::new();
    let mut index = 0;
    let base_path = RdPath::new(Vec::new());

    while index < nodes.len() {
        match &nodes[index] {
            RdNode::RCode(code) => output.push_str(code),
            RdNode::Comment(_) => {}
            node => {
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

    output
}

fn try_format_infix_call(generic: &str, call_text: &str) -> Option<String> {
    let leading_len = call_text.len() - call_text.trim_start().len();
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
    use rd_ast::{RdNode, RdTag};

    use super::convert_usage;

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
}
