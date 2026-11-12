//! Access to the Fortitude formatter for the LSP

use fortitude_formatter::{cached_formatter, format_lines, format_text};
use ruff_source_file::LineIndex;
use ruff_text_size::{TextRange, TextSize};

use crate::{
    PositionEncoding,
    edit::{Replacement, ToRangeExt},
    resolve::is_document_excluded_for_formatting,
    session::DocumentQuery,
};

/// Format the whole document, returning the edits to apply to it, or `None` if
/// formatting doesn't change it or the document is excluded from formatting
pub(crate) fn format(
    query: &DocumentQuery,
    encoding: PositionEncoding,
) -> crate::Result<Option<Vec<lsp_types::TextEdit>>> {
    if is_excluded(query) {
        return Ok(None);
    }

    let document = query.as_single_document();
    let source = document.contents();
    let settings = &query.settings().format;
    let language = cached_formatter(settings.indent_width);
    let formatted = format_text(source, &language, settings).map_err(formatter_error)?;
    if formatted == source {
        return Ok(None);
    }

    Ok(Some(vec![minimal_edit(
        source,
        document.index(),
        &formatted,
        encoding,
    )]))
}

/// Format the statements on the lines of `range`, returning the edits to apply
/// to the document, or `None` if formatting doesn't change them or the document
/// is excluded from formatting
pub(crate) fn format_range(
    query: &DocumentQuery,
    range: lsp_types::Range,
    encoding: PositionEncoding,
) -> crate::Result<Option<Vec<lsp_types::TextEdit>>> {
    if is_excluded(query) {
        return Ok(None);
    }

    let document = query.as_single_document();
    let source = document.contents();
    let settings = &query.settings().format;
    let language = cached_formatter(settings.indent_width);
    let edits =
        format_lines(source, &language, settings, range_lines(range)).map_err(formatter_error)?;
    if edits.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        edits
            .into_iter()
            .map(|edit| {
                let range = TextRange::new(
                    TextSize::try_from(edit.source_range.start)?,
                    TextSize::try_from(edit.source_range.end)?,
                );
                Ok(lsp_types::TextEdit {
                    range: range.to_range(source, document.index(), encoding),
                    new_text: edit.new_text,
                })
            })
            .collect::<crate::Result<_>>()?,
    ))
}

fn is_excluded(query: &DocumentQuery) -> bool {
    is_document_excluded_for_formatting(
        &query.virtual_file_path(),
        &query.settings().file_resolver,
        query.text_document_language_id(),
    )
}

/// `FormatterError` can't be sent between threads, so only keep its message
fn formatter_error(err: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("Failed to format document: {err}")
}

/// The lines touched by `range`, numbered from 0. A range ending at the start
/// of a line, such as a selection of whole lines, doesn't include that line.
fn range_lines(range: lsp_types::Range) -> std::ops::Range<usize> {
    let start = range.start.line as usize;
    let end = if range.end.character == 0 && range.end.line > range.start.line {
        range.end.line as usize
    } else {
        range.end.line as usize + 1
    };
    start..end
}

/// A single edit replacing the lines of `source` that differ from `formatted`
fn minimal_edit(
    source: &str,
    source_index: &LineIndex,
    formatted: &str,
    encoding: PositionEncoding,
) -> lsp_types::TextEdit {
    let formatted_index = LineIndex::from_source_text(formatted);
    let Replacement {
        source_range,
        modified_range,
    } = Replacement::between(
        source,
        source_index.line_starts(),
        formatted,
        formatted_index.line_starts(),
    );
    lsp_types::TextEdit {
        range: source_range.to_range(source, source_index, encoding),
        new_text: formatted[modified_range].to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use lsp_types::{Position, Range};
    use ruff_source_file::LineIndex;

    use super::{minimal_edit, range_lines};
    use crate::PositionEncoding;

    #[test]
    fn edit_only_covers_changed_lines() {
        let source = "program p
  integer :: i
  do i = 1, 3
print *, i
  end do
end program p
";
        let formatted = "program p
  integer :: i
  do i = 1, 3
    print *, i
  end do
end program p
";
        let edit = minimal_edit(
            source,
            &LineIndex::from_source_text(source),
            formatted,
            PositionEncoding::UTF16,
        );
        assert_eq!(edit.range.start, Position::new(3, 0));
        assert_eq!(edit.range.end, Position::new(4, 0));
        assert_eq!(edit.new_text, "    print *, i\n");
    }

    #[test]
    fn lines_of_range() {
        let range = |start: (u32, u32), end: (u32, u32)| {
            range_lines(Range::new(
                Position::new(start.0, start.1),
                Position::new(end.0, end.1),
            ))
        };
        // A cursor position
        assert_eq!(range((3, 4), (3, 4)), 3..4);
        // Part of a line
        assert_eq!(range((3, 0), (3, 4)), 3..4);
        // Whole lines, ending at the start of the next line
        assert_eq!(range((3, 0), (5, 0)), 3..5);
        // Ending within a line
        assert_eq!(range((3, 2), (5, 1)), 3..6);
    }
}
