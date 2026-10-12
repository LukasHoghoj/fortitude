//! Keeping trailing comments at the end of their statement.
//!
//! The formatter puts a line break after every statement, which would move a
//! trailing comment to the next line. Trailing comments are therefore
//! replaced by marker comments on the following line before formatting, and
//! put back at the end of the statement afterwards.

use crate::continuation::scan_line;

const MARKER: &str = "!fortitude-format-comment:";
/// Marks comment lines starting in the first column
const COLUMN0_MARKER: &str = "!fortitude-format-column0:";

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Mark comment lines starting in the first column, so that
/// [`unmark_column0_comments`] can put them there again after formatting
pub(crate) fn mark_column0_comments(lines: Vec<String>) -> Vec<String> {
    let mut in_string = None;
    lines
        .into_iter()
        .map(|line| {
            // Markers of the formatter itself are indented by it
            let is_comment = in_string.is_none()
                && line.starts_with('!')
                && !line.starts_with("!fortitude-format-");
            in_string = scan_line(&line, in_string).open_string;
            if is_comment {
                format!("{COLUMN0_MARKER}{line}")
            } else {
                line
            }
        })
        .collect()
}

/// Put the comment lines marked by [`mark_column0_comments`] in the first
/// column again
pub(crate) fn unmark_column0_comments(lines: Vec<String>) -> Vec<String> {
    let mut result = Vec::with_capacity(lines.len());
    for line in lines {
        match line.split_once(COLUMN0_MARKER) {
            Some((code, comment)) => {
                // The formatter may have put the comment behind code
                if !code.trim().is_empty() {
                    result.push(code.trim_end().to_string());
                }
                result.push(comment.to_string());
            }
            None => result.push(line),
        }
    }
    result
}

/// Move trailing comments of code lines to marker comments on the next line.
/// Returns the lines and the comments removed.
pub(crate) fn detach_trailing_comments(lines: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut result = Vec::with_capacity(lines.len());
    let mut comments = vec![];
    let mut in_string = None;
    for line in lines {
        let scan = scan_line(&line, in_string);
        in_string = scan.open_string;
        let code = &line[..scan.code_end];
        if code.trim().is_empty() || scan.code_end == line.len() {
            result.push(line);
            continue;
        }
        let indent = " ".repeat(indent_of(&line));
        result.push(code.trim_end().to_string());
        result.push(format!("{indent}{MARKER}{}", comments.len()));
        comments.push(line[scan.code_end..].to_string());
    }
    (result, comments)
}

/// Put the comments removed by [`detach_trailing_comments`] back at the end
/// of their statement. Comments that would make the line longer than
/// `line_length` are put on a line of their own after the statement, indented
/// like it, if they fit there or if they are going to be wrapped
/// (`split_comments`).
pub(crate) fn attach_trailing_comments(
    lines: Vec<String>,
    comments: &[String],
    line_length: usize,
    split_comments: bool,
) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    // Index of the first line of the last statement
    let mut statement_start = 0;
    let mut continued = false;
    for line in lines {
        let Some((code, id)) = line
            .split_once(MARKER)
            .and_then(|(code, id)| Some((code.trim_end(), id.trim().parse::<usize>().ok()?)))
        else {
            if !continued {
                statement_start = result.len();
            }
            let scan = scan_line(&line, None);
            continued = line[..scan.code_end].trim_end().ends_with('&');
            result.push(line);
            continue;
        };
        let comment = &comments[id];

        let indent = if code.trim().is_empty() {
            result.get(statement_start).map_or(0, |l| indent_of(l))
        } else {
            indent_of(code)
        };
        let target = if code.trim().is_empty() {
            result.last_mut()
        } else {
            // The formatter put the marker behind the code
            statement_start = result.len();
            result.push(code.to_string());
            result.last_mut()
        };
        let Some(target) = target else {
            result.push(comment.clone());
            continue;
        };
        if target.len() + 1 + comment.len() > line_length
            && (split_comments || indent + comment.len() <= line_length)
        {
            result.push(format!("{}{comment}", " ".repeat(indent)));
        } else {
            target.push(' ');
            target.push_str(comment);
        }
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
    fn column0_comments() {
        let lines = mark_column0_comments(strings(&[
            "! header",
            "  ! indented",
            "print *, \"abc &",
            "! not a comment\"",
            "!fortitude-format-directive:0",
        ]));
        assert_eq!(
            lines,
            [
                &format!("{COLUMN0_MARKER}! header"),
                "  ! indented",
                "print *, \"abc &",
                "! not a comment\"",
                "!fortitude-format-directive:0",
            ]
        );
        // After formatting
        let formatted = strings(&[
            &format!("  {COLUMN0_MARKER}! header"),
            "    ! indented",
            &format!("  do i = 1, n {COLUMN0_MARKER}! behind code"),
        ]);
        assert_eq!(
            unmark_column0_comments(formatted),
            [
                "! header",
                "    ! indented",
                "  do i = 1, n",
                "! behind code"
            ]
        );
    }

    #[test]
    fn detach() {
        let (lines, comments) = detach_trailing_comments(strings(&[
            "  a = 1 ! one",
            "  ! standalone",
            "  print *, 'b ! not a comment' ! two",
            "  print *, \"c &",
            "&! in string\" ! three",
        ]));
        assert_eq!(
            lines,
            [
                "  a = 1",
                &format!("  {MARKER}0"),
                "  ! standalone",
                "  print *, 'b ! not a comment'",
                &format!("  {MARKER}1"),
                "  print *, \"c &",
                "&! in string\"",
                &format!("{MARKER}2"),
            ]
        );
        assert_eq!(comments, ["! one", "! two", "! three"]);
    }

    #[test]
    fn attach() {
        let comments = strings(&["! one", "! two", "! a rather long comment"]);
        let lines = strings(&[
            "  a = 1",
            &format!("  {MARKER}0"),
            &format!("  do i = 1, n {MARKER}1"),
            "    call foo(a, &",
            "      b)",
            &format!("    {MARKER}2"),
        ]);
        assert_eq!(
            attach_trailing_comments(lines.clone(), &comments, 100, false),
            [
                "  a = 1 ! one",
                "  do i = 1, n ! two",
                "    call foo(a, &",
                "      b) ! a rather long comment",
            ]
        );
        assert_eq!(
            attach_trailing_comments(lines.clone(), &comments, 20, true),
            [
                "  a = 1 ! one",
                "  do i = 1, n ! two",
                "    call foo(a, &",
                "      b)",
                "    ! a rather long comment",
            ]
        );
        // Without wrapping, a comment only gets a line of its own if it fits there
        assert_eq!(
            attach_trailing_comments(lines.clone(), &comments, 27, false),
            [
                "  a = 1 ! one",
                "  do i = 1, n ! two",
                "    call foo(a, &",
                "      b)",
                "    ! a rather long comment",
            ]
        );
        assert_eq!(
            attach_trailing_comments(lines, &comments, 20, false),
            [
                "  a = 1 ! one",
                "  do i = 1, n ! two",
                "    call foo(a, &",
                "      b) ! a rather long comment",
            ]
        );
    }
}
