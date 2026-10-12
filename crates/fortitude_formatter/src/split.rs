//! Splitting of string literals and comment lines exceeding the line length.

use crate::continuation::scan_line;

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Whether the code of `line` ends with a continuation '&'
fn is_continued(line: &str, in_string: Option<char>) -> bool {
    let scan = scan_line(line, in_string);
    scan.open_string.is_some() || line[..scan.code_end].trim_end().ends_with('&')
}

/// For each byte of `line`, the delimiter of the string literal it is part of
/// (excluding the delimiters themselves). `in_string` is the delimiter of a
/// string literal continued from the previous line.
fn string_contents(line: &str, in_string: Option<char>) -> Vec<Option<char>> {
    let bytes = line.as_bytes();
    let mut states = vec![None; bytes.len()];
    let mut in_string = in_string.map(|c| c as u8);
    let mut pos = 0;
    while pos < bytes.len() {
        let c = bytes[pos];
        match in_string {
            Some(delim) if c == delim => {
                if bytes.get(pos + 1) == Some(&delim) {
                    // Escaped delimiter
                    states[pos] = Some(delim as char);
                    states[pos + 1] = Some(delim as char);
                    pos += 1;
                } else {
                    in_string = None;
                }
            }
            Some(delim) => states[pos] = Some(delim as char),
            None if c == b'"' || c == b'\'' => in_string = Some(c),
            None if c == b'!' => break,
            None => {}
        }
        pos += 1;
    }
    states
}

/// Find a position within a string literal of `line` at which the string can
/// be continued on the next line ('...&' / '&...'), with the first part
/// `line[..pos]` not longer than `max_pos`. Prefer splitting after
/// whitespace, unless that leaves less than half of the line.
fn find_string_split(line: &str, max_pos: usize, in_string: Option<char>) -> Option<usize> {
    let states = string_contents(line, in_string);
    let bytes = line.as_bytes();
    let min_pos = indent_of(line) + 1;
    let mut best = None;
    for pos in (min_pos..=max_pos.min(states.len().saturating_sub(1))).rev() {
        // Both neighbours must be ordinary characters of the string
        let (Some(delim), Some(_)) = (states[pos], states[pos - 1]) else {
            continue;
        };
        let ordinary = |c: u8| c != delim as u8 && c != b'&';
        if !ordinary(bytes[pos]) || !ordinary(bytes[pos - 1]) || !line.is_char_boundary(pos) {
            continue;
        }
        let best = *best.get_or_insert(pos);
        if bytes[pos - 1] == b' ' {
            if pos >= best / 2 {
                return Some(pos);
            }
            break;
        }
    }
    best
}

/// Split string literals of lines exceeding `line_length`, using character
/// context continuation, which leaves the value of the string unchanged.
pub(crate) fn split_long_strings(lines: &mut Vec<String>, line_length: usize, indent: usize) {
    let mut in_string = None;
    let mut continued = false;
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if line.len() > line_length {
            if let Some(pos) = find_string_split(line, line_length.saturating_sub(1), in_string) {
                // The first line of a statement opens a new level of indentation
                let next_indent = indent_of(line) + if continued { 0 } else { indent };
                let rest = format!("{}&{}", " ".repeat(next_indent), &line[pos..]);
                lines[i].truncate(pos);
                lines[i].push('&');
                lines.insert(i + 1, rest);
            }
        }
        continued = is_continued(&lines[i], in_string);
        in_string = scan_line(&lines[i], in_string).open_string;
        i += 1;
    }
}

/// Find the last blank outside of string literals and comments at which
/// `line` can be broken, so that the first part followed by ' &' is not longer
/// than `line_length`. The part after the break must contain code.
fn find_blank_split(line: &str, line_length: usize, in_string: Option<char>) -> Option<usize> {
    let scan = scan_line(line, in_string);
    let code = line[..scan.code_end].trim_end();
    // Don't separate a trailing '&' from the code in front of it
    let code = match scan.open_string {
        Some(_) => return None,
        None => code.strip_suffix('&').unwrap_or(code).trim_end(),
    };
    let states = string_contents(line, in_string);
    let bytes = code.as_bytes();
    let min_pos = indent_of(line) + 1;
    let max_pos = line_length
        .saturating_sub(2)
        .min(code.len().saturating_sub(1));
    (min_pos..=max_pos).rev().find(|&pos| {
        bytes[pos] == b' '
            && states[pos].is_none()
            && !code[..pos].trim_end().ends_with('&')
            && !code[pos..].trim().is_empty()
    })
}

/// Break lines exceeding `line_length` at blanks outside of string literals
/// and comments, as a last resort if the syntax gives no better place.
pub(crate) fn split_at_blanks(lines: &mut Vec<String>, line_length: usize, indent: usize) {
    let mut in_string = None;
    let mut continued = false;
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        let is_comment = in_string.is_none() && line.trim_start().starts_with('!');
        if line.len() > line_length && !is_comment {
            if let Some(pos) = find_blank_split(line, line_length, in_string) {
                // The first line of a statement opens a new level of indentation
                let next_indent = indent_of(line) + if continued { 0 } else { indent };
                let rest = format!("{}{}", " ".repeat(next_indent), line[pos..].trim_start());
                let first = format!("{} &", lines[i][..pos].trim_end());
                lines[i] = first;
                lines.insert(i + 1, rest);
            }
        }
        continued = is_continued(&lines[i], in_string);
        in_string = scan_line(&lines[i], in_string).open_string;
        i += 1;
    }
}

/// Comments that must not be wrapped: compiler and OpenMP/OpenACC directives
/// (`!$omp`, `!$`, `!DIR$`, ...), '!&' and `! allow(...)` comments
fn is_unsplittable_comment(comment: &str) -> bool {
    let Some(rest) = comment.strip_prefix('!') else {
        return true;
    };
    let word: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    rest.starts_with('$')
        || rest.starts_with('&')
        || rest[word.len()..].starts_with('$')
        || rest.trim_start().starts_with("allow(")
}

/// Wrap the comment `line` at whitespace so that each part fits within
/// `line_length`. The comment marker (e.g. '! ', '!> ') is repeated on every
/// part, and the indentation kept. Returns None if the comment must not or
/// can't be split.
fn split_comment(line: &str, line_length: usize) -> Option<Vec<String>> {
    let indent = &line[..indent_of(line)];
    let comment = line.trim();
    if is_unsplittable_comment(comment) {
        return None;
    }

    let marker_len = comment.chars().take_while(|&c| c == '!').count();
    let marker_len = marker_len
        + usize::from(matches!(
            comment[marker_len..].chars().next(),
            Some('<' | '>')
        ));
    let prefix_len = comment.len() - comment[marker_len..].trim_start().len();
    let first_prefix = &comment[..prefix_len];
    let prefix = if prefix_len > marker_len {
        first_prefix.to_string()
    } else {
        format!("{first_prefix} ")
    };

    let width = line_length.checked_sub(indent.len() + prefix.len())?;
    if width < 10 {
        return None;
    }

    let mut text = &comment[prefix_len..];
    let mut parts = vec![];
    while text.len() > width {
        let Some(pos) = text[..=width].rfind(' ').filter(|&p| p > 0) else {
            break;
        };
        parts.push(text[..pos].trim_end());
        text = text[pos..].trim_start();
    }
    if parts.is_empty() {
        return None;
    }
    parts.push(text);

    Some(
        parts
            .into_iter()
            .enumerate()
            .map(|(i, part)| {
                let prefix = if i == 0 { first_prefix } else { &prefix };
                format!("{indent}{prefix}{part}")
            })
            .collect(),
    )
}

/// Wrap comment lines exceeding `line_length`
pub(crate) fn split_long_comments(lines: &mut Vec<String>, line_length: usize) {
    let mut in_string = None;
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if in_string.is_none() && line.trim_start().starts_with('!') && line.len() > line_length {
            if let Some(parts) = split_comment(line, line_length) {
                let n = parts.len();
                lines.splice(i..=i, parts);
                i += n;
                continue;
            }
        }
        in_string = scan_line(&lines[i], in_string).open_string;
        i += 1;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn split_string() {
        let mut lines = vec![
            r#"  print *, "short", "this one has a bang! and ""escaped quotes"" intact", 42"#
                .to_string(),
        ];
        split_long_strings(&mut lines, 50, 2);
        assert_eq!(
            lines,
            [
                r#"  print *, "short", "this one has a bang! &"#,
                r#"    &and ""escaped quotes"" intact", 42"#,
            ]
        );
    }

    #[test]
    fn split_string_without_spaces() {
        let mut lines = vec![
            "  call log(&".to_string(),
            "    \"averyveryveryveryverylongwordwithoutanyspacesinsideatall\")".to_string(),
        ];
        split_long_strings(&mut lines, 40, 2);
        assert_eq!(
            lines,
            [
                "  call log(&",
                "    \"averyveryveryveryverylongwordwitho&",
                "    &utanyspacesinsideatall\")",
            ]
        );
    }

    #[test]
    fn no_split_outside_strings() {
        let mut lines = vec!["  x = 1 ! \"a comment that is long enough to be split\"".to_string()];
        split_long_strings(&mut lines, 20, 2);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn split_blanks() {
        let mut lines = vec![
            "  print *, 'a string with blanks', 'another one', x == y".to_string(),
            "  call foo(first, second, & ! comment".to_string(),
            "    third)".to_string(),
        ];
        split_at_blanks(&mut lines, 30, 2);
        assert_eq!(
            lines,
            [
                "  print *, &",
                "    'a string with blanks', &",
                "    'another one', x == y",
                "  call foo(first, &",
                "    second, & ! comment",
                "    third)",
            ]
        );
    }

    #[test]
    fn split_comments() {
        let mut lines = vec![
            "  ! a long indented comment line that is wider than the line limit".to_string(),
            "!> Doxygen documentation for the subroutine that needs wrapping too.".to_string(),
            "!no space after the marker but still a long comment to be wrapped".to_string(),
            "!DIR$ ATTRIBUTES FORCEINLINE :: some_extremely_long_routine_name_x".to_string(),
            "!$omp parallel do private(some_extremely_long_variable_name, another_one)".to_string(),
            "! allow(line-too-long) because the following line is really very long".to_string(),
            format!("! {}", "x".repeat(70)),
        ];
        split_long_comments(&mut lines, 60);
        assert_eq!(
            lines,
            [
                "  ! a long indented comment line that is wider than the line",
                "  ! limit",
                "!> Doxygen documentation for the subroutine that needs",
                "!> wrapping too.",
                "!no space after the marker but still a long comment to be",
                "! wrapped",
                "!DIR$ ATTRIBUTES FORCEINLINE :: some_extremely_long_routine_name_x",
                "!$omp parallel do private(some_extremely_long_variable_name, another_one)",
                "! allow(line-too-long) because the following line is really very long",
                &format!("! {}", "x".repeat(70)),
            ]
        );
    }

    #[test]
    fn no_comment_split_in_continued_string() {
        let mut lines = vec![
            "  print *, \"abc &".to_string(),
            "  &! this is not a comment but part of the string and long\"".to_string(),
        ];
        let expected = lines.clone();
        split_long_comments(&mut lines, 30);
        assert_eq!(lines, expected);
    }
}
