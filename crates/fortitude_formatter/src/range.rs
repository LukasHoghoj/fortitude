//! Formatting only some lines of a file
//!
//! The whole file is formatted, so that indentation and line breaks have their
//! full context, and only the changes touching the requested lines are kept.
//! Lines of the original and formatted text are matched ignoring whitespace, so
//! a line that is only indented or spaced differently is changed on its own.
//! Lines that are joined, split or removed are changed together. The requested
//! lines are first widened to whole statements, including all their
//! continuation lines, and several ranges of lines are merged where they
//! overlap or meet.
//!
//! The formatted lines are indented like the code before them, so that
//! formatting part of a file indented differently from the formatter's settings
//! doesn't leave it inconsistent. Their indentation is that of the last
//! statement before their range of lines, plus the change in indentation the
//! formatter makes from that statement, in the file's own indentation width.

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

/// Format `source`, returning only the edits touching the statements on the
/// given ranges of `lines` (numbered from 0), which may be in any order and
/// overlap. The edits are ordered and don't overlap.
pub fn format_lines(
    source: &str,
    language: &Language,
    settings: &FormatSettings,
    lines: &[Range<usize>],
) -> Result<Vec<LineEdit>, FormatterError> {
    let formatted = format_text(source, language, settings)?;
    Ok(line_edits(
        source,
        &formatted,
        lines,
        crate::indent_width(language),
    ))
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
/// the given ranges of `lines` of `source`, indented like the code before
/// them. `indent_width` is the formatter's indentation width.
fn line_edits(
    source: &str,
    formatted: &str,
    lines: &[Range<usize>],
    indent_width: usize,
) -> Vec<LineEdit> {
    let old: Vec<&str> = source.split_inclusive('\n').collect();
    let new: Vec<&str> = formatted.split_inclusive('\n').collect();
    let ranges = widen_and_merge(&old, lines);
    if ranges.is_empty() {
        return Vec::new();
    }

    let old_keys: Vec<String> = old.iter().map(|line| without_whitespace(line)).collect();
    let new_keys: Vec<String> = new.iter().map(|line| without_whitespace(line)).collect();
    let ops = capture_diff_slices(Algorithm::Patience, &old_keys, &new_keys);

    let reindent = Reindent::new(&old, &new, &ops, indent_width);
    // Each range is indented like the code before it
    let anchors: Vec<_> = ranges
        .iter()
        .map(|range| reindent.anchor(range.start))
        .collect();
    // The index of the first range ending after `line`, if any
    let first_range_after = |line: usize| ranges.partition_point(|range| range.end <= line);

    let line_offsets: Vec<usize> = std::iter::once(0)
        .chain(old.iter().scan(0, |offset, line| {
            *offset += line.len();
            Some(*offset)
        }))
        .collect();
    let edit = |old_lines: Range<usize>, new_lines: Range<usize>, anchor| LineEdit {
        source_range: line_offsets[old_lines.start]..line_offsets[old_lines.end],
        new_text: new_lines.map(|line| reindent.line(line, anchor)).collect(),
        lines: old_lines,
    };

    let mut edits = Vec::new();
    for op in ops {
        match op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                // Lines differing only in whitespace are changed one by one
                for i in 0..len {
                    let (old_line, new_line) = (old_index + i, new_index + i);
                    let index = first_range_after(old_line);
                    if ranges
                        .get(index)
                        .is_some_and(|range| range.contains(&old_line))
                    {
                        let anchor = anchors[index];
                        let new_text = reindent.line(new_line, anchor);
                        if old[old_line] != new_text {
                            edits.push(edit(
                                old_line..old_line + 1,
                                new_line..new_line + 1,
                                anchor,
                            ));
                        }
                    }
                }
            }
            _ => {
                let (old_lines, new_lines) = (op.old_range(), op.new_range());
                // The first range the changed lines touch, which may be one of
                // several they span
                let index = first_range_after(old_lines.start);
                let touches = ranges.get(index).is_some_and(|range| {
                    if old_lines.is_empty() {
                        range.contains(&old_lines.start)
                    } else {
                        range.start < old_lines.end
                    }
                });
                if touches {
                    edits.push(edit(old_lines, new_lines, anchors[index]));
                }
            }
        }
    }
    edits
}

/// The indentation of the statement that formatted lines are indented
/// relative to, in the source and in the formatted text
type Anchor = Option<(usize, usize)>;

/// Indents formatted lines like the source before a formatted range
struct Reindent<'a> {
    old: &'a [&'a str],
    new: &'a [&'a str],
    /// The formatted line of each source line whose text is unchanged
    formatted_line: Vec<Option<usize>>,
    /// Whether each source line is the first line of a statement
    old_starts: Vec<bool>,
    /// Whether each formatted line starts within a character context
    in_string: Vec<bool>,
    /// The first line of the statement on each formatted line
    statement_start: Vec<usize>,
    /// The indentation width of the source
    source_width: usize,
    /// The indentation width of the formatter
    formatter_width: usize,
}

impl<'a> Reindent<'a> {
    fn new(old: &'a [&'a str], new: &'a [&'a str], ops: &[DiffOp], formatter_width: usize) -> Self {
        // The formatted line of each source line whose text is unchanged
        let mut formatted_line = vec![None; old.len()];
        for op in ops {
            if let DiffOp::Equal {
                old_index,
                new_index,
                len,
            } = *op
            {
                for i in 0..len {
                    formatted_line[old_index + i] = Some(new_index + i);
                }
            }
        }

        let old_starts = statement_starts(old);
        let new_continuation = continuation(new);
        let mut statement_start = Vec::with_capacity(new.len());
        for line in 0..new.len() {
            statement_start.push(if line > 0 && new_continuation[line - 1].continued {
                statement_start[line - 1]
            } else {
                line
            });
        }

        Self {
            old,
            new,
            formatted_line,
            in_string: new_continuation.iter().map(|c| c.in_string).collect(),
            statement_start,
            source_width: source_indent_width(old, &old_starts).unwrap_or(formatter_width),
            old_starts,
            formatter_width: formatter_width.max(1),
        }
    }

    /// The anchor of a range starting at the source line `start`: the last
    /// statement before it that the formatter keeps
    fn anchor(&self, start: usize) -> Anchor {
        (0..start.min(self.old.len())).rev().find_map(|line| {
            let new_line = self.formatted_line[line]?;
            self.old_starts[line]
                .then(|| (indentation(self.old[line]), indentation(self.new[new_line])))
        })
    }

    /// The formatted line `line`, indented like the source relative to
    /// `anchor`
    fn line(&self, line: usize, anchor: Anchor) -> String {
        let text = self.new[line];
        match self.indent(line, anchor) {
            Some(indent) => format!("{}{}", " ".repeat(indent), text.trim_start()),
            None => text.to_string(),
        }
    }

    /// The indentation of the formatted line `line`, or `None` to keep it as
    /// formatted
    fn indent(&self, line: usize, anchor: Anchor) -> Option<usize> {
        let (source_anchor, formatted_anchor) = anchor?;
        let text = self.new[line];
        let content = text.trim_start();
        let indent = indentation(text);
        if content.trim().is_empty() {
            return None;
        }
        if self.in_string[line] {
            // Whitespace continuing a string without a leading `&` is part of
            // the string. Otherwise, the formatter keeps the alignment of the
            // `&`, so keep it relative to the start of the statement.
            if !content.starts_with('&') {
                return None;
            }
            let start = self.statement_start[line];
            let start_indent = indentation(self.new[start]);
            let base = self.indent(start, anchor).unwrap_or(start_indent);
            return Some(base + indent.saturating_sub(start_indent));
        }
        // Lines in the first column, such as program units and preprocessor
        // directives, stay there
        if indent == 0 {
            return None;
        }
        let change = (indent as isize - formatted_anchor as isize) * self.source_width as isize;
        let change = change / self.formatter_width as isize;
        Some((source_anchor as isize + change).max(0) as usize)
    }
}

/// The number of columns of leading whitespace of `line`
fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

/// Whether `line` has code on it, rather than being blank, a comment or a
/// preprocessor directive
fn is_code(line: &str) -> bool {
    let content = line.trim_start();
    !(content.trim().is_empty() || content.starts_with('!') || content.starts_with('#'))
}

/// For each line, whether it is the first line of a statement with code on it
fn statement_starts(lines: &[&str]) -> Vec<bool> {
    let continued = continued_lines(lines);
    (0..lines.len())
        .map(|line| is_code(lines[line]) && (line == 0 || !continued[line - 1]))
        .collect()
}

/// The indentation width used in the source: the most common increase in
/// indentation from one statement to the next, the smallest if several are
/// as common
fn source_indent_width(lines: &[&str], starts: &[bool]) -> Option<usize> {
    let mut counts = std::collections::BTreeMap::new();
    let mut previous = None;
    for (line, _) in lines.iter().zip(starts).filter(|(_, start)| **start) {
        let indent = indentation(line);
        if let Some(previous) = previous
            && indent > previous
        {
            *counts.entry(indent - previous).or_insert(0) += 1;
        }
        previous = Some(indent);
    }
    // `max_by_key` returns the last maximum, so iterate from the largest width
    counts
        .into_iter()
        .rev()
        .max_by_key(|&(_, count)| count)
        .map(|(width, _)| width)
}

fn without_whitespace(line: &str) -> String {
    line.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Widen each of `ranges` to include the whole statements on its lines, and
/// merge those that overlap or meet, returning them in order
fn widen_and_merge(lines: &[&str], ranges: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut widened: Vec<_> = ranges
        .iter()
        .map(|range| widen_to_statements(lines, range.clone()))
        .collect();
    widened.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(widened.len());
    for range in widened {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
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
    continuation(lines).iter().map(|c| c.continued).collect()
}

/// How a line relates to the statements continued around it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Continuation {
    /// The statement on this line continues on the next line
    continued: bool,
    /// This line starts within a character context continued from the
    /// previous line
    in_string: bool,
}

fn continuation(lines: &[&str]) -> Vec<Continuation> {
    let mut result = Vec::with_capacity(lines.len());
    // Whether the previous line was continued, and the quote of a character
    // context it was continued in
    let mut previous = false;
    let mut open_quote = None;

    for line in lines {
        let in_string = open_quote.is_some();
        let mut chars = line.trim_start().chars().peekable();
        // Like comments, preprocessor directives don't end a continued statement
        if open_quote.is_none() && chars.peek() == Some(&'#') {
            result.push(Continuation {
                continued: previous,
                in_string,
            });
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
        result.push(Continuation {
            continued: is_continued,
            in_string,
        });
    }
    result
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
        format_ranges(source, &[lines])
    }

    /// Format the given ranges of lines of `source`, returning the result
    fn format_ranges(source: &str, lines: &[std::ops::Range<usize>]) -> String {
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
            format_lines(&formatted, &language, &settings(), &[0..usize::MAX]).unwrap(),
            vec![]
        );
        // Lines formatted already aren't changed, even if others are
        assert_eq!(
            format_lines(SOURCE, &language, &settings(), &[0..1]).unwrap(),
            vec![]
        );
    }

    #[test]
    fn formats_several_ranges() {
        // Lines 4 and 6, but not line 5 between them
        assert_eq!(
            format_ranges(SOURCE, &[5..6, 3..4]),
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
    fn overlapping_and_adjacent_ranges_are_merged() {
        for ranges in [&[3..5, 4..6][..], &[3..4, 4..6], &[4..6, 3..5, 3..4]] {
            assert_eq!(format_ranges(SOURCE, ranges), format_range(SOURCE, 3..6));
        }
        assert_eq!(format_ranges(SOURCE, &[]), SOURCE);
    }

    #[test]
    fn each_range_is_indented_like_the_code_before_it() {
        let source = "module m
contains
    subroutine s(x)
        integer :: x
x=1
    end subroutine s
  subroutine t(y)
    integer :: y
y=2
  end subroutine t
end module m
";
        // Indented by 4 in `s`, and by 2 in `t`
        assert_eq!(
            format_ranges(source, &[4..5, 8..9]),
            "module m
contains
    subroutine s(x)
        integer :: x
        x = 1
    end subroutine s
  subroutine t(y)
    integer :: y
    y = 2
  end subroutine t
end module m
"
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
    /// formatting the whole file. Their indentation is removed first, as lines
    /// are otherwise indented like the fixture rather than the formatter.
    #[test]
    fn line_by_line_matches_whole_file() {
        let language = cached_formatter(2);
        let fixtures =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/test/fixtures/format");
        let mut checked = 0;
        for entry in std::fs::read_dir(fixtures).unwrap() {
            let path = entry.unwrap().path();
            let source: String = std::fs::read_to_string(&path)
                .unwrap()
                .split_inclusive('\n')
                .map(|line| line.trim_start_matches([' ', '\t']))
                .collect();
            let expected = format_text(&source, &language, &settings()).unwrap();

            let mut text = source.clone();
            let mut line = 0;
            while line < text.lines().count() {
                let edits = format_lines(&text, &language, &settings(), &[line..line + 1]).unwrap();
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
    fn matches_source_indent_width() {
        let source = "module m
contains
    subroutine s(x)
        integer :: x
        do x = 1, 3
x=x+1
        end do
    end subroutine s
end module m
";
        // Indented by 4, like the code before it, rather than by 2
        assert_eq!(
            format_range(source, 5..6),
            "module m
contains
    subroutine s(x)
        integer :: x
        do x = 1, 3
            x = x + 1
        end do
    end subroutine s
end module m
"
        );
    }

    #[test]
    fn matches_inconsistent_indentation() {
        let source = "program p
integer :: i
do i = 1, 3
   print *, i
   if (i > 1) then
print *, 'big'
   end if
end do
end program p
";
        // One level of 3 inside the `if`, although the program's body isn't
        // indented at all
        assert_eq!(
            format_range(source, 5..6),
            "program p
integer :: i
do i = 1, 3
   print *, i
   if (i > 1) then
      print *, 'big'
   end if
end do
end program p
"
        );
    }

    #[test]
    fn keeps_string_continuation_and_first_column() {
        let source = "module m
contains
    subroutine s
        character(len=20) :: c
c='abc&
  def'
c='abc&
         &def'
!$omp barrier
    end subroutine s
end module m
";
        // The alignment of `&` continuing a string is kept relative to the
        // statement, rather than scaled like indentation, and the directive
        // stays in the first column
        assert_eq!(
            format_range(source, 4..9),
            "module m
contains
    subroutine s
        character(len=20) :: c
        c = 'abc&
          &  def'
        c = 'abc&
                 &def'
!$omp barrier
    end subroutine s
end module m
"
        );
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
