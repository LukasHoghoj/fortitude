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

const MARKER: &str = "!fortitude-format-continued:";

/// A line break within a statement, kept because of comments
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LineBreak {
    /// Number of non-blank characters of the statement in front of the break
    offset: usize,
    /// Comment at the end of the line
    comment: Option<String>,
    /// Comment lines between this line and the next one
    comment_lines: Vec<String>,
    /// Whether the next line starts with '&'
    leading_ampersand: bool,
}

/// A statement whose line breaks are restored after formatting, see
/// [`restore_line_breaks`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeptStatement {
    breaks: Vec<LineBreak>,
    /// Comment at the end of the last line
    comment: String,
    /// The non-blank characters of the statement, as passed to the formatter
    nonblank: String,
    /// Number of lines of the statement passed to the formatter (more than
    /// one if it contains continued strings)
    rows: usize,
}

/// Result of [`join_continuation_lines`]
#[derive(Debug, Default)]
pub(crate) struct Joined {
    pub lines: Vec<String>,
    pub kept: Vec<KeptStatement>,
}

/// A logical line being assembled from continuation lines
#[derive(Default)]
struct Statement {
    text: String,
    indent: usize,
    breaks: Vec<LineBreak>,
    /// Comment lines that can't stay where they are
    moved_comments: Vec<String>,
}

impl Statement {
    fn has_comments(&self) -> bool {
        self.breaks
            .iter()
            .any(|b| b.comment.is_some() || !b.comment_lines.is_empty())
    }

    /// Record a line break at the end of the text so far
    fn add_break(&mut self, comment: &str) {
        self.breaks.push(LineBreak {
            offset: count_nonblank(&self.text),
            comment: (!comment.is_empty()).then(|| comment.to_string()),
            ..Default::default()
        });
    }
}

fn count_nonblank(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
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
/// A `!` within a string literal does not start a comment. The line breaks
/// of statements with comments on continued lines, or comment lines between
/// continuation lines, are kept: such statements are followed by a marker
/// comment, and the line breaks and comments are put back by
/// [`restore_line_breaks`] after formatting.
pub(crate) fn join_continuation_lines(source: &str, join_strings: bool) -> Joined {
    let mut result = Joined::default();
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
                    let mut statement = Statement {
                        text: code.to_string(),
                        indent: indent_of(line),
                        ..Default::default()
                    };
                    if scan.open_string.is_none() {
                        statement.add_break(comment);
                    }
                    current = Some(statement);
                    in_string = scan.open_string;
                }
                // Comment or blank line
                _ if code.trim().is_empty() => result.lines.push(line.to_string()),
                // Not continued
                _ => {
                    let statement = Statement {
                        text: code.trim_start().to_string(),
                        indent: indent_of(line),
                        ..Default::default()
                    };
                    emit_statement(statement, &line[scan.code_end..], &mut result);
                }
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
                match statement.breaks.last_mut() {
                    Some(line_break) => line_break.comment_lines.push(trimmed.to_string()),
                    None => statement.moved_comments.push(trimmed.to_string()),
                }
            }
            current = Some(statement);
            continue;
        }

        let rest = match strip_leading_ampersand(line) {
            // A token may be continued directly after a leading '&'
            Some(rest) => {
                if let Some(line_break) = statement.breaks.last_mut() {
                    line_break.leading_ampersand = true;
                }
                rest
            }
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
    result: &mut Joined,
) {
    let text_start = statement.text.len() - rest.len();
    let code = rest[..scan.code_end].trim_end();
    let comment = &rest[scan.code_end..];

    match code.strip_suffix('&') {
        Some(code) => {
            let code = if scan.open_string.is_some() {
                code
            } else {
                code.trim_end()
            };
            statement.text.truncate(text_start + code.len());
            if scan.open_string.is_none() {
                statement.add_break(comment);
            }
            *current = Some(std::mem::take(statement));
        }
        None => {
            statement.text.truncate(text_start + code.len());
            emit_statement(std::mem::take(statement), comment, result);
        }
    }
}

/// Split `text` into the statements separated by ';' (outside of string literals)
fn split_statements(text: &str) -> Vec<&str> {
    let mut statements = vec![];
    let mut start = 0;
    let mut in_string: Option<char> = None;
    let mut chars = text.char_indices().peekable();
    while let Some((pos, c)) = chars.next() {
        match in_string {
            Some(delim) if c == delim => {
                if chars.peek().is_some_and(|&(_, next)| next == delim) {
                    chars.next();
                } else {
                    in_string = None;
                }
            }
            Some(_) => {}
            None if c == '"' || c == '\'' => in_string = Some(c),
            None if c == ';' => {
                statements.push(&text[start..pos]);
                start = pos + 1;
            }
            None => {}
        }
    }
    statements.push(&text[start..]);
    statements
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Add a complete statement.
///
/// Statements separated by ';' are put on lines of their own, as a line can't
/// start with ';'. If a statement with comments on its continued lines can't
/// keep its line breaks, the comments are put in front of it.
fn emit_statement(statement: Statement, comment: &str, result: &mut Joined) {
    let indent = " ".repeat(statement.indent);
    for moved in &statement.moved_comments {
        result.lines.push(format!("{indent}{moved}"));
    }

    let statements = split_statements(&statement.text);
    if statement.has_comments() && statements.len() == 1 {
        let text = statements[0];
        let mut lines = text.split('\n');
        result
            .lines
            .push(format!("{indent}{}", lines.next().unwrap_or_default()));
        result.lines.extend(lines.map(String::from));
        result
            .lines
            .push(format!("{indent}{MARKER}{}", result.kept.len()));
        result.kept.push(KeptStatement {
            breaks: statement.breaks,
            comment: comment.to_string(),
            nonblank: text.chars().filter(|c| !c.is_whitespace()).collect(),
            rows: text.split('\n').count(),
        });
        return;
    }

    for line_break in &statement.breaks {
        for moved in line_break.comment.iter().chain(&line_break.comment_lines) {
            result.lines.push(format!("{indent}{moved}"));
        }
    }
    let start = result.lines.len();
    for text in statements {
        let mut lines = text.split('\n');
        result
            .lines
            .push(format!("{indent}{}", lines.next().unwrap_or_default()));
        result.lines.extend(lines.map(String::from));
    }
    if !comment.is_empty() {
        if result.lines.len() == start {
            result.lines.push(format!("{indent}{comment}"));
        } else {
            let last = result.lines.last_mut().unwrap();
            last.push(' ');
            last.push_str(comment);
        }
    }
}

/// Byte position in `text` right after its `n`-th non-blank character
fn position_after_nonblank(text: &str, n: usize) -> Option<usize> {
    if n == 0 {
        return Some(0);
    }
    text.char_indices()
        .filter(|(_, c)| !c.is_whitespace())
        .nth(n - 1)
        .map(|(pos, c)| pos + c.len_utf8())
}

fn is_word_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

/// Split the formatted `text` of a statement at its original line breaks.
/// Returns None if the formatted statement doesn't match the original one.
fn split_kept_statement(
    text: &str,
    kept: &KeptStatement,
    indent: usize,
    indent_width: usize,
) -> Option<Vec<String>> {
    let nonblank: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if nonblank != kept.nonblank {
        return None;
    }

    // Positions of the breaks, and whether they are within a token, which
    // requires a leading '&' and no blank in front of the trailing one
    let mut ends = vec![];
    let mut within_token = vec![];
    let mut start = 0;
    for line_break in &kept.breaks {
        let end = position_after_nonblank(text, line_break.offset)?.max(start);
        within_token.push(
            line_break.leading_ampersand
                && is_word_char(text[..end].chars().last())
                && is_word_char(text[end..].trim_start().chars().next()),
        );
        ends.push(end);
        start = end;
    }
    ends.push(text.len());

    let continuation_indent = " ".repeat(indent + indent_width);
    let mut lines: Vec<String> = vec![];
    let mut start = 0;
    for (i, &end) in ends.iter().enumerate() {
        let piece = &text[start..end];
        let piece = if i == 0 {
            piece.trim_end()
        } else {
            piece.trim()
        };
        let mut rows = piece.split('\n');
        let first = rows.next().unwrap_or_default();
        if i == 0 {
            lines.push(first.to_string());
        } else {
            let ampersand = match (kept.breaks[i - 1].leading_ampersand, within_token[i - 1]) {
                (false, _) => "",
                (true, true) => "&",
                (true, false) => "& ",
            };
            lines.push(format!("{continuation_indent}{ampersand}{first}"));
        }
        lines.extend(rows.map(String::from));

        let last = lines.last_mut().unwrap();
        match kept.breaks.get(i) {
            Some(line_break) => {
                last.push_str(if within_token[i] { "&" } else { " &" });
                if let Some(comment) = &line_break.comment {
                    last.push(' ');
                    last.push_str(comment);
                }
                for comment in &line_break.comment_lines {
                    lines.push(format!("{continuation_indent}{comment}"));
                }
            }
            None => {
                if !kept.comment.is_empty() {
                    last.push(' ');
                    last.push_str(&kept.comment);
                }
            }
        }
        start = end;
    }
    Some(lines)
}

/// Put back the line breaks and comments of the statements kept by
/// [`join_continuation_lines`] into the formatted `lines`. Continuation lines
/// are indented by `indent_width` relative to the start of the statement.
pub(crate) fn restore_line_breaks(
    lines: Vec<String>,
    kept: &[KeptStatement],
    indent_width: usize,
) -> Vec<String> {
    if kept.is_empty() {
        return lines;
    }
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    for line in lines {
        let Some((code, id)) = line
            .split_once(MARKER)
            .and_then(|(code, id)| Some((code.trim_end(), id.trim().parse::<usize>().ok()?)))
        else {
            result.push(line);
            continue;
        };
        if !code.trim().is_empty() {
            // The formatter put the marker behind the code
            result.push(code.to_string());
        }
        let kept = &kept[id];
        let rows = kept.rows.min(result.len());
        let statement = result.split_off(result.len() - rows);
        let text = statement.join("\n");
        let indent = indent_of(&text);
        match split_kept_statement(&text, kept, indent, indent_width) {
            Some(split) => result.extend(split),
            None => {
                // Keep the formatted statement, with the comments in front of it
                let indent = " ".repeat(indent);
                for line_break in &kept.breaks {
                    for comment in line_break.comment.iter().chain(&line_break.comment_lines) {
                        result.push(format!("{indent}{comment}"));
                    }
                }
                result.extend(statement);
                if !kept.comment.is_empty() {
                    let last = result.last_mut().unwrap();
                    last.push(' ');
                    last.push_str(&kept.comment);
                }
            }
        }
    }
    result
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
            join_continuation_lines(source, false).lines,
            vec![
                "print *, \"abc &",
                "   &mark! and &",
                "   &more\", 42 ! end"
            ]
        );
        assert_eq!(
            join_continuation_lines(source, true).lines,
            vec!["print *, \"abc mark! and more\", 42 ! end"]
        );
    }

    #[test]
    fn keep_line_breaks_with_comments() {
        let source = "  call foo(a, & ! the first\n  ! between\n    b, &\n\n    & c) ! last\n";
        let joined = join_continuation_lines(source, false);
        assert_eq!(
            joined.lines,
            vec!["  call foo(a, b, c)", &format!("  {MARKER}0")]
        );
        // Formatting may change the whitespace
        let formatted = vec![
            "    call foo(a,  b,c)".to_string(),
            format!("    {MARKER}0"),
            "    x = 1".to_string(),
        ];
        assert_eq!(
            restore_line_breaks(formatted, &joined.kept, 2),
            vec![
                "    call foo(a, & ! the first",
                "      ! between",
                "      b, &",
                "      & c) ! last",
                "    x = 1",
            ]
        );
    }

    #[test]
    fn keep_line_breaks_within_token() {
        let source = "x = some_long_na& ! comment\n  &me + 1\n";
        let joined = join_continuation_lines(source, false);
        let formatted = vec!["x = some_long_name + 1".to_string(), format!("{MARKER}0")];
        assert_eq!(
            restore_line_breaks(formatted, &joined.kept, 2),
            vec!["x = some_long_na& ! comment", "  &me + 1"]
        );
    }

    #[test]
    fn keep_line_breaks_mismatch() {
        let source = "call foo(a, & ! the first\n  b)\n";
        let joined = join_continuation_lines(source, false);
        // The formatter changed more than whitespace: keep its result
        let formatted = vec!["call foo(a, c)".to_string(), format!("{MARKER}0")];
        assert_eq!(
            restore_line_breaks(formatted, &joined.kept, 2),
            vec!["! the first", "call foo(a, c)"]
        );
    }

    #[test]
    fn move_comments_of_multiple_statements() {
        let source = "a = 1; call foo(a, & ! the first\n  b) ! last\n";
        assert_eq!(
            join_continuation_lines(source, false).lines,
            vec!["! the first", "a = 1", "call foo(a, b) ! last"]
        );
    }

    #[test]
    fn join_tokens() {
        let source = "real &\n  function f()\ninte&\n  &ger :: i\n! comment &\nx = 1\n";
        assert_eq!(
            join_continuation_lines(source, false).lines,
            vec!["real function f()", "integer :: i", "! comment &", "x = 1"]
        );
    }

    #[test]
    fn split_at_semicolons() {
        let source = "  a = 1; print *, 'a;b', \"c &\n  &;d\"; ; b = 2 ! comment;\n";
        assert_eq!(
            join_continuation_lines(source, false).lines,
            vec![
                "  a = 1",
                "  print *, 'a;b', \"c &",
                "&;d\"",
                "  b = 2 ! comment;"
            ]
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
