//! Formatting only some lines of a file
//!
//! The whole file is formatted, so that indentation and line breaks have their
//! full context, and only the changes touching the requested lines are kept.
//! Lines of the original and formatted text are matched ignoring whitespace, so
//! a line that is only indented or spaced differently is changed on its own.
//! Lines that are joined, split or removed are changed together. The requested
//! lines are first widened to whole statements, including all their
//! continuation lines.

use std::ops::Range;

use similar::{Algorithm, DiffOp, capture_diff_slices};
use topiary_core::{FormatterError, Language};

use fortitude_linter::settings::FormatSettings;

use crate::format_source;

/// Format `source`, keeping its line endings. Unless it is empty, the result
/// ends with a line ending.
pub fn format_text(
    source: &str,
    language: &Language,
    settings: &FormatSettings,
) -> Result<String, FormatterError> {
    let lines = format_source(source, language, settings)?;
    let newline = line_ending(source);
    let mut formatted = lines.join(newline);
    if !formatted.is_empty() {
        formatted.push_str(newline);
    }
    Ok(formatted)
}

/// The line ending of the first line of `source`, `\n` if it has none
fn line_ending(source: &str) -> &'static str {
    match source.find('\n') {
        Some(newline) if source[..newline].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

/// A replacement of whole lines of the source by formatted lines
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineEdit {
    /// The replaced lines, numbered from 0
    pub lines: Range<usize>,
    /// The byte offsets of the replaced lines in the source, including their
    /// line endings
    pub source_range: Range<usize>,
    /// The formatted lines, including their line endings
    pub new_text: String,
}

/// Format `source`, returning only the edits touching the given `lines`
/// (numbered from 0) of the statements on these lines. The edits are ordered
/// and don't overlap.
pub fn format_lines(
    source: &str,
    language: &Language,
    settings: &FormatSettings,
    lines: Range<usize>,
) -> Result<Vec<LineEdit>, FormatterError> {
    let formatted = format_text(source, language, settings)?;
    Ok(line_edits(source, &formatted, lines))
}

/// Apply `edits`, as returned by [`format_lines`], to `source`
pub fn apply_line_edits(source: &str, edits: &[LineEdit]) -> String {
    let mut result = String::with_capacity(source.len());
    let mut last = 0;
    for edit in edits {
        result.push_str(&source[last..edit.source_range.start]);
        result.push_str(&edit.new_text);
        last = edit.source_range.end;
    }
    result.push_str(&source[last..]);
    result
}

/// The edits turning `source` into `formatted` that touch the statements on
/// the given `lines` of `source`
fn line_edits(source: &str, formatted: &str, lines: Range<usize>) -> Vec<LineEdit> {
    let old: Vec<&str> = source.split_inclusive('\n').collect();
    let new: Vec<&str> = formatted.split_inclusive('\n').collect();
    let lines = widen_to_statements(&old, lines);

    let line_offsets: Vec<usize> = std::iter::once(0)
        .chain(old.iter().scan(0, |offset, line| {
            *offset += line.len();
            Some(*offset)
        }))
        .collect();
    let edit = |old_lines: Range<usize>, new_lines: Range<usize>| LineEdit {
        source_range: line_offsets[old_lines.start]..line_offsets[old_lines.end],
        new_text: new[new_lines].concat(),
        lines: old_lines,
    };

    let old_keys: Vec<String> = old.iter().map(|line| without_whitespace(line)).collect();
    let new_keys: Vec<String> = new.iter().map(|line| without_whitespace(line)).collect();

    let mut edits = Vec::new();
    for op in capture_diff_slices(Algorithm::Patience, &old_keys, &new_keys) {
        match op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                // Lines differing only in whitespace are changed one by one
                for i in 0..len {
                    let (old_line, new_line) = (old_index + i, new_index + i);
                    if lines.contains(&old_line) && old[old_line] != new[new_line] {
                        edits.push(edit(old_line..old_line + 1, new_line..new_line + 1));
                    }
                }
            }
            _ => {
                let (old_lines, new_lines) = (op.old_range(), op.new_range());
                let touches = if old_lines.is_empty() {
                    lines.contains(&old_lines.start)
                } else {
                    old_lines.start < lines.end && lines.start < old_lines.end
                };
                if touches {
                    edits.push(edit(old_lines, new_lines));
                }
            }
        }
    }
    edits
}

fn without_whitespace(line: &str) -> String {
    line.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Widen `lines` to include the whole statements on these lines
fn widen_to_statements(lines: &[&str], range: Range<usize>) -> Range<usize> {
    let continued = continued_lines(lines);
    let start = range.start.min(lines.len());
    let end = range.end.clamp(start, lines.len());

    let start = (0..start)
        .rev()
        .find(|&line| !continued[line])
        .map_or(0, |line| line + 1);
    let end = (end.max(start + 1).min(lines.len())..lines.len())
        .find(|&line| !continued[line - 1])
        .unwrap_or(lines.len());
    start..end.max(start)
}

/// For each line, whether the statement on it continues on the next line. This
/// is the case if the line ends with `&`, outside of comments, or if it is a
/// comment or blank line within a continued statement.
fn continued_lines(lines: &[&str]) -> Vec<bool> {
    let mut continued = Vec::with_capacity(lines.len());
    // Whether the previous line was continued, and the quote of a character
    // context it was continued in
    let mut previous = false;
    let mut open_quote = None;

    for line in lines {
        let mut chars = line.trim_start().chars().peekable();
        // Like comments, preprocessor directives don't end a continued statement
        if open_quote.is_none() && chars.peek() == Some(&'#') {
            continued.push(previous);
            continue;
        }
        if open_quote.is_some() && chars.peek() == Some(&'&') {
            chars.next();
        }

        let mut quote = open_quote;
        let mut last = None;
        while let Some(c) = chars.next() {
            match quote {
                Some(q) if c == q => {
                    // A doubled quote is an escaped quote
                    if chars.peek() == Some(&q) {
                        chars.next();
                    } else {
                        quote = None;
                    }
                }
                Some(_) => {}
                None if c == '!' => break,
                None if c == '\'' || c == '"' => quote = Some(c),
                None => {}
            }
            if !c.is_whitespace() {
                last = Some(c);
            }
        }

        let is_continued = match last {
            Some(c) => c == '&',
            // Comment and blank lines don't end a continued statement
            None => previous,
        };
        open_quote = if is_continued { quote } else { None };
        previous = is_continued;
        continued.push(is_continued);
    }
    continued
}

#[cfg(test)]
mod tests {
    use fortitude_linter::settings::{FormatSettings, PreviewMode};

    use super::{apply_line_edits, continued_lines, format_lines, format_text};
    use crate::cached_formatter;

    fn settings() -> FormatSettings {
        FormatSettings {
            preview: PreviewMode::Enabled,
            ..FormatSettings::default()
        }
    }

    /// Format `lines` of `source`, returning the result
    fn format_range(source: &str, lines: std::ops::Range<usize>) -> String {
        let language = cached_formatter(2);
        let edits = format_lines(source, &language, &settings(), lines).unwrap();
        apply_line_edits(source, &edits)
    }

    const SOURCE: &str = "program p
integer :: i
do i = 1, 3
print *, i
end do
x=1
end program p
";

    #[test]
    fn keeps_line_endings() {
        let language = cached_formatter(2);
        let formatted = format_text(&SOURCE.replace('\n', "\r\n"), &language, &settings());
        assert_eq!(
            formatted.unwrap(),
            "program p\r\n  integer :: i\r\n  do i = 1, 3\r\n    print *, i\r\n  end do\r\n  x = 1\r\nend program p\r\n"
        );
        assert_eq!(format_text("", &language, &settings()).unwrap(), "");
    }

    #[test]
    fn formats_single_line_with_context() {
        assert_eq!(
            format_range(SOURCE, 3..4),
            "program p
integer :: i
do i = 1, 3
    print *, i
end do
x=1
end program p
"
        );
    }

    #[test]
    fn formats_several_lines() {
        assert_eq!(
            format_range(SOURCE, 4..6),
            "program p
integer :: i
do i = 1, 3
print *, i
  end do
  x = 1
end program p
"
        );
    }

    #[test]
    fn whole_range_is_whole_file() {
        let language = cached_formatter(2);
        assert_eq!(
            format_range(SOURCE, 0..usize::MAX),
            format_text(SOURCE, &language, &settings()).unwrap()
        );
    }

    #[test]
    fn empty_range_formats_its_line() {
        assert_eq!(format_range(SOURCE, 1..1), format_range(SOURCE, 1..2));
    }

    #[test]
    fn formatted_lines_are_unchanged() {
        let language = cached_formatter(2);
        let formatted = format_text(SOURCE, &language, &settings()).unwrap();
        assert_eq!(
            format_lines(&formatted, &language, &settings(), 0..usize::MAX).unwrap(),
            vec![]
        );
        // Lines formatted already aren't changed, even if others are
        assert_eq!(
            format_lines(SOURCE, &language, &settings(), 0..1).unwrap(),
            vec![]
        );
    }

    #[test]
    fn widens_to_continued_statement() {
        let source = "program p
x = 1 + &
2 + &
! comment
3
y=2
end program p
";
        // Only the middle of the statement is in the range
        assert_eq!(
            format_range(source, 2..3),
            "program p
  x = 1 + &
    2 + &
    ! comment
    3
y=2
end program p
"
        );
    }

    /// Formatting each line of the fixtures in turn gives the same result as
    /// formatting the whole file
    #[test]
    fn line_by_line_matches_whole_file() {
        let language = cached_formatter(2);
        let fixtures =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/test/fixtures/format");
        let mut checked = 0;
        for entry in std::fs::read_dir(fixtures).unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();
            let expected = format_text(&source, &language, &settings()).unwrap();

            let mut text = source.clone();
            let mut line = 0;
            while line < text.lines().count() {
                let edits = format_lines(&text, &language, &settings(), line..line + 1).unwrap();
                // Continue with the line after this one, or after the lines
                // replacing it, in the edited text
                let mut next = line + 1;
                for edit in &edits {
                    let new_len = edit.new_text.lines().count();
                    if edit.lines.end <= line + 1 {
                        next = next + new_len - edit.lines.len();
                    } else if edit.lines.start <= line + 1 {
                        next = next.max(edit.lines.start + new_len);
                    }
                }
                text = apply_line_edits(&text, &edits);
                line = next;
            }
            assert_eq!(text, expected, "{}", path.display());
            checked += 1;
        }
        assert!(checked > 30);
    }

    #[test]
    fn continuation_lines() {
        let lines = [
            "x = 'a & b' ! c &\n",
            "y = 'abc &\n",
            "  &def ! not a comment &\n",
            "  &ghi' &\n",
            "\n",
            "! comment\n",
            "  + 1\n",
            "#ifdef X\n",
            "z = 'it''s' &\n",
            "#ifdef Y\n",
            "  // 's'\n",
        ];
        assert_eq!(
            continued_lines(&lines),
            vec![
                false, true, true, true, true, true, false, false, true, true, false
            ]
        );
    }
}
