//! Keeping trailing comments at the end of their statement.
//!
//! The formatter puts a line break after every statement, which would move a
//! trailing comment to the next line. Trailing comments are therefore
//! replaced by marker comments on the following line before formatting, and
//! put back at the end of the statement afterwards.

use crate::continuation::scan_line;

const MARKER: &str = "!fortitude-format-comment:";

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
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
/// of their statement. If `line_length` is given, comments that would make
/// the line too long are put on a line of their own after the statement,
/// indented like it.
pub(crate) fn attach_trailing_comments(
    lines: Vec<String>,
    comments: &[String],
    line_length: Option<usize>,
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
        if line_length.is_some_and(|max| target.len() + 1 + comment.len() > max) {
            let indent = indent_of(&result[statement_start]);
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
            attach_trailing_comments(lines.clone(), &comments, None),
            [
                "  a = 1 ! one",
                "  do i = 1, n ! two",
                "    call foo(a, &",
                "      b) ! a rather long comment",
            ]
        );
        assert_eq!(
            attach_trailing_comments(lines, &comments, Some(20)),
            [
                "  a = 1 ! one",
                "  do i = 1, n ! two",
                "    call foo(a, &",
                "      b)",
                "    ! a rather long comment",
            ]
        );
    }
}
