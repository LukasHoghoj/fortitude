//! Joining of continuation lines into single lines before formatting, and
//! restoring the layout of continued string literals afterwards.

/// Result of scanning a single physical line of free-form Fortran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineScan {
    /// Byte index at which a trailing comment starts (or the line length)
    pub code_end: usize,
    /// Delimiter of a string literal still open at the end of the line
    pub open_string: Option<char>,
}

/// Scan `line` for string literals and comments. `in_string` is the
/// delimiter of a string literal continued from the previous line.
pub(crate) fn scan_line(line: &str, in_string: Option<char>) -> LineScan {
    let mut in_string = in_string;
    let mut chars = line.char_indices().peekable();
    while let Some((pos, c)) = chars.next() {
        match in_string {
            Some(delim) if c == delim => {
                // A doubled delimiter is an escaped delimiter
                if chars.peek().is_some_and(|&(_, next)| next == delim) {
                    chars.next();
                } else {
                    in_string = None;
                }
            }
            Some(_) => {}
            None if c == '"' || c == '\'' => in_string = Some(c),
            None if c == '!' => {
                return LineScan {
                    code_end: pos,
                    open_string: None,
                }
            }
            None => {}
        }
    }
    LineScan {
        code_end: line.len(),
        open_string: in_string,
    }
}

/// Removes a leading continuation `&` (and the whitespace before it)
fn strip_leading_ampersand(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('&')
}

/// Length of the leading whitespace of `line`
fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// A logical line being assembled from continuation lines
#[derive(Default)]
struct Statement {
    text: String,
    indent: usize,
    /// Comments from lines other than the last one, which can't stay where they are
    comments: Vec<String>,
}

/// Join continuation lines so that each Fortran statement is on one line.
///
/// String literals continued over several lines are kept intact, including
/// the continuation characters: their continuation lines are put on separate
/// lines indented relative to the start of the statement, see
/// [`indent_string_continuations`]. With `join_strings`, the parts of a
/// continued string are joined instead, which leaves the value of the string
/// unchanged.
///
/// A `!` within a string literal does not start a comment. Comments on lines
/// that are continued, or between continuation lines, are moved in front of
/// the statement.
pub(crate) fn join_continuation_lines(source: &str, join_strings: bool) -> Vec<String> {
    let mut result = Vec::new();
    // Statement continued from the previous line, and string literal open at its end
    let mut current: Option<Statement> = None;
    let mut in_string: Option<char> = None;

    for line in source.lines() {
        let line = line.trim_end();

        let Some(mut statement) = current.take() else {
            // Start of a new statement
            let scan = scan_line(line, None);
            let code = line[..scan.code_end].trim_end();
            match code.strip_suffix('&') {
                Some(code) if !code.trim().is_empty() || scan.open_string.is_some() => {
                    let comment = &line[scan.code_end..];
                    // Keep the whitespace in front of the '&' of a continued string
                    let code = if scan.open_string.is_some() {
                        code.trim_start()
                    } else {
                        code.trim()
                    };
                    current = Some(Statement {
                        text: code.to_string(),
                        indent: indent_of(line),
                        comments: if comment.is_empty() {
                            vec![]
                        } else {
                            vec![comment.to_string()]
                        },
                    });
                    in_string = scan.open_string;
                }
                // Not continued, or a comment / blank line
                _ => result.push(line.to_string()),
            }
            continue;
        };

        // Continuation line
        if let Some(delim) = in_string {
            // Within a continued string literal, which must continue directly after a leading '&'
            let rest = strip_leading_ampersand(line).unwrap_or(line);
            if join_strings {
                statement.text.push_str(rest);
            } else {
                let indent = indent_of(line).saturating_sub(statement.indent);
                statement.text.push_str("&\n");
                statement.text.push_str(&" ".repeat(indent));
                statement.text.push('&');
                statement.text.push_str(rest);
            }
            let scan = scan_line(rest, Some(delim));
            in_string = scan.open_string;
            finish_line(&mut statement, rest, &scan, &mut current, &mut result);
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('!') {
            // Comment or blank line between continuation lines
            if !trimmed.is_empty() {
                statement.comments.push(trimmed.to_string());
            }
            current = Some(statement);
            continue;
        }

        let rest = match strip_leading_ampersand(line) {
            // A token may be continued directly after a leading '&'
            Some(rest) => rest,
            None => {
                statement.text.push(' ');
                trimmed
            }
        };
        statement.text.push_str(rest);
        let scan = scan_line(rest, None);
        in_string = scan.open_string;
        finish_line(&mut statement, rest, &scan, &mut current, &mut result);
    }

    // Unterminated continuation at the end of the file
    if let Some(statement) = current {
        emit_statement(statement, "", &mut result);
    }

    result
}

/// Handle the end of the continuation line `rest` (already appended to the
/// statement): either the statement is continued, or it is complete.
fn finish_line(
    statement: &mut Statement,
    rest: &str,
    scan: &LineScan,
    current: &mut Option<Statement>,
    result: &mut Vec<String>,
) {
    let text_start = statement.text.len() - rest.len();
    let code = rest[..scan.code_end].trim_end();
    let comment = rest[scan.code_end..].to_string();

    match code.strip_suffix('&') {
        Some(code) => {
            let code = if scan.open_string.is_some() {
                code
            } else {
                code.trim_end()
            };
            statement.text.truncate(text_start + code.len());
            if !comment.is_empty() {
                statement.comments.push(comment);
            }
            *current = Some(std::mem::take(statement));
        }
        None => {
            statement.text.truncate(text_start + code.len());
            emit_statement(std::mem::take(statement), &comment, result);
        }
    }
}

/// Add a complete statement, preceded by comments moved from continuation lines
fn emit_statement(statement: Statement, comment: &str, result: &mut Vec<String>) {
    let indent = " ".repeat(statement.indent);
    for moved in &statement.comments {
        result.push(format!("{indent}{moved}"));
    }
    let mut lines: Vec<String> = statement.text.split('\n').map(String::from).collect();
    lines[0].insert_str(0, &indent);
    if !comment.is_empty() {
        let last = lines.last_mut().unwrap();
        last.push(' ');
        last.push_str(comment);
    }
    result.extend(lines);
}

/// Indent the continuation lines of continued string literals relative to the
/// start of their statement. The continuation lines have to be indented
/// relative to column 0 already, as created by [`join_continuation_lines`].
pub(crate) fn indent_string_continuations(lines: &mut [String]) {
    let mut in_string = None;
    let mut statement_indent = 0;
    let mut continued = false;
    for line in lines.iter_mut() {
        if in_string.is_some() {
            line.insert_str(0, &" ".repeat(statement_indent));
        } else if !continued {
            statement_indent = indent_of(line);
        }
        let scan = scan_line(line, in_string);
        in_string = scan.open_string;
        continued = line[..scan.code_end].trim_end().ends_with('&');
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn scan_bang_in_string() {
        let scan = scan_line(r#"print *, "a ! b", 'c'' ! d' ! comment"#, None);
        assert_eq!(scan.code_end, 28);
        assert_eq!(scan.open_string, None);

        let scan = scan_line(r#"   &mark! and &"#, Some('"'));
        assert_eq!(scan.code_end, 15);
        assert_eq!(scan.open_string, Some('"'));
    }

    #[test]
    fn join_continued_string() {
        let source = "print *, \"abc &\n   &mark! and &\n   &more\", 42 ! end\n";
        assert_eq!(
            join_continuation_lines(source, false),
            vec![
                "print *, \"abc &",
                "   &mark! and &",
                "   &more\", 42 ! end"
            ]
        );
        assert_eq!(
            join_continuation_lines(source, true),
            vec!["print *, \"abc mark! and more\", 42 ! end"]
        );
    }

    #[test]
    fn join_with_comments() {
        let source = "  call foo(a, & ! first\n  ! between\n    b, &\n    & c) ! last\n";
        assert_eq!(
            join_continuation_lines(source, false),
            vec!["  ! first", "  ! between", "  call foo(a, b, c) ! last"]
        );
    }

    #[test]
    fn join_tokens() {
        let source = "real &\n  function f()\ninte&\n  &ger :: i\n! comment &\nx = 1\n";
        assert_eq!(
            join_continuation_lines(source, false),
            vec!["real function f()", "integer :: i", "! comment &", "x = 1"]
        );
    }

    #[test]
    fn indent_continued_string() {
        let mut lines = vec![
            "  print *, \"abc &".to_string(),
            "   &mark! and &".to_string(),
            "&more\", 42".to_string(),
            "  call foo(a, &".to_string(),
            "    b)".to_string(),
        ];
        indent_string_continuations(&mut lines);
        assert_eq!(
            lines,
            vec![
                "  print *, \"abc &",
                "     &mark! and &",
                "  &more\", 42",
                "  call foo(a, &",
                "    b)"
            ]
        );
    }
}
