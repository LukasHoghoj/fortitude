//! Keeping lines the parser can't make sense of as they are.
//!
//! A line consisting only of a parse error (e.g. invalid code) would be
//! merged with the surrounding statements by the formatter. Such lines are
//! replaced by marker comments before formatting, and put back afterwards,
//! indented like the code around them.

const MARKER: &str = "!fortitude-format-error:";

/// Statements which don't open or close a block, so that keeping them as
/// they are doesn't affect the indentation of the code around them
const SIMPLE_STATEMENTS: &[&str] = &[
    "allocate_statement",
    "assignment_statement",
    "close_statement",
    "deallocate_statement",
    "file_position_statement",
    "format_statement",
    "implicit_statement",
    "inquire_statement",
    "keyword_statement",
    "nullify_statement",
    "open_statement",
    "pointer_association_statement",
    "print_statement",
    "read_statement",
    "stop_statement",
    "subroutine_call",
    "use_statement",
    "variable_declaration",
    "variable_modification",
    "write_statement",
];

/// Rows of the lines to keep as they are: lines with a parse error within
/// the line, which consist only of the error (not starting with a word) or
/// are simple statements
fn error_rows(source: &str) -> Vec<usize> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_fortran::LANGUAGE.into())
        .expect("Error loading Fortran parser");
    let Some(tree) = parser.parse(source, None) else {
        return vec![];
    };
    let lines: Vec<&str> = source.split('\n').collect();
    let line_starts: Vec<usize> = lines
        .iter()
        .scan(0, |start, line| {
            let current = *start;
            *start += line.len() + 1;
            Some(current)
        })
        .collect();

    let mut rows = vec![];
    let mut cursor = tree.walk();
    let mut visit = true;
    loop {
        let node = cursor.node();
        if visit && node.is_error() {
            let (start, end) = (node.start_position(), node.end_position());
            // The error may include the line break
            let single_line = start.row == end.row || (end.row == start.row + 1 && end.column == 0);
            if single_line {
                let row = start.row;
                let line = lines[row];
                let code_start = line_starts[row] + line.len() - line.trim_start().len();
                // A line starting with a word may be the opening or closing
                // statement of a block, which the formatter must see
                let only_error = node.start_byte() <= code_start
                    && !line.trim_start().starts_with(|c: char| c.is_alphabetic());
                if only_error || is_simple_statement(&tree, code_start) {
                    rows.push(row);
                }
            }
        }
        // Errors within errors are covered already
        if visit && !node.is_error() && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            visit = true;
            continue;
        }
        if !cursor.goto_parent() {
            break;
        }
        visit = false;
    }
    rows
}

/// Whether the statement starting at byte `start` is a simple statement
fn is_simple_statement(tree: &tree_sitter::Tree, start: usize) -> bool {
    let Some(mut node) = tree.root_node().descendant_for_byte_range(start, start) else {
        return false;
    };
    while let Some(parent) = node.parent() {
        if parent.start_byte() != start || parent.parent().is_none() {
            break;
        }
        node = parent;
    }
    SIMPLE_STATEMENTS.contains(&node.kind())
}

/// Replace lines with parse errors by marker comments, see [`error_rows`].
/// Returns the lines and the original text of the replaced lines.
pub(crate) fn mark_error_lines(lines: Vec<String>) -> (Vec<String>, Vec<String>) {
    let rows = error_rows(&lines.join("\n"));
    if rows.is_empty() {
        return (lines, vec![]);
    }

    let mut result = Vec::with_capacity(lines.len());
    let mut originals = vec![];
    for (row, line) in lines.into_iter().enumerate() {
        if rows.contains(&row) && !line.trim().is_empty() {
            let indent = &line[..line.len() - line.trim_start().len()];
            result.push(format!("{indent}{MARKER}{}", originals.len()));
            originals.push(line.trim().to_string());
        } else {
            result.push(line);
        }
    }
    (result, originals)
}

/// Put back the lines replaced by [`mark_error_lines`]
pub(crate) fn unmark_error_lines(lines: Vec<String>, originals: &[String]) -> Vec<String> {
    if originals.is_empty() {
        return lines;
    }
    let mut result = Vec::with_capacity(lines.len());
    for line in lines {
        let Some((code, id)) = line
            .split_once(MARKER)
            .and_then(|(code, id)| Some((code, id.trim().parse::<usize>().ok()?)))
        else {
            result.push(line);
            continue;
        };
        let indent = if code.trim().is_empty() {
            code.len()
        } else {
            // The formatter put the marker behind code
            result.push(code.trim_end().to_string());
            code.len() - code.trim_start().len()
        };
        result.push(format!("{}{}", " ".repeat(indent), originals[id]));
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;

    fn strings(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn error_line() {
        let lines = strings(&[
            "program p",
            "  print *, \"abc \"",
            "  \"def\"",
            "  print *, 1",
            "end program",
        ]);
        let (marked, originals) = mark_error_lines(lines);
        assert_eq!(marked[2], format!("  {MARKER}0"));
        assert_eq!(originals, ["\"def\""]);
        let formatted = strings(&[
            "program p",
            "    print *, \"abc \"",
            &format!("    {MARKER}0"),
            &format!("    print *, 1 {MARKER}0"),
        ]);
        assert_eq!(
            unmark_error_lines(formatted, &originals),
            [
                "program p",
                "    print *, \"abc \"",
                "    \"def\"",
                "    print *, 1",
                "    \"def\""
            ]
        );
    }

    #[test]
    fn error_in_simple_statement() {
        let lines = strings(&["program p", "  print *, \"abc \" \"def\"", "end program"]);
        let (marked, originals) = mark_error_lines(lines);
        assert_eq!(marked[1], format!("  {MARKER}0"));
        assert_eq!(originals, ["print *, \"abc \" \"def\""]);
    }

    #[test]
    fn valid_code() {
        let lines = strings(&["program p", "  print *, 1", "end program"]);
        let (marked, originals) = mark_error_lines(lines.clone());
        assert_eq!(marked, lines);
        assert!(originals.is_empty());
    }
}
