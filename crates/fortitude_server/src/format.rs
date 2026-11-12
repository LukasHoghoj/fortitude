//! Access to the Fortitude formatter for the LSP

use fortitude_formatter::{cached_formatter, format_source};
use fortitude_linter::settings::FormatSettings;
use ruff_source_file::LineIndex;

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
    let settings = query.settings();
    let document_path = query.virtual_file_path();

    if is_document_excluded_for_formatting(
        &document_path,
        &settings.file_resolver,
        query.text_document_language_id(),
    ) {
        return Ok(None);
    }

    let document = query.as_single_document();
    let source = document.contents();
    let Some(formatted) = format_text(source, &settings.format)? else {
        return Ok(None);
    };

    Ok(Some(vec![minimal_edit(
        source,
        document.index(),
        &formatted,
        encoding,
    )]))
}

/// Format `source`, returning `None` if formatting doesn't change it. The
/// formatted text keeps the line endings of `source`.
pub(crate) fn format_text(
    source: &str,
    settings: &FormatSettings,
) -> crate::Result<Option<String>> {
    let language = cached_formatter(settings.indent_width);
    // `FormatterError` can't be sent between threads, so only keep its message
    let lines = format_source(source, &language, settings)
        .map_err(|err| anyhow::anyhow!("Failed to format document: {err}"))?;

    let newline = if uses_crlf(source) { "\r\n" } else { "\n" };
    let mut formatted = lines.join(newline);
    if !formatted.is_empty() {
        formatted.push_str(newline);
    }

    Ok((formatted != source).then_some(formatted))
}

/// Whether the first line of `source` ends with `\r\n`
fn uses_crlf(source: &str) -> bool {
    source
        .find('\n')
        .is_some_and(|newline| source[..newline].ends_with('\r'))
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
    use fortitude_linter::settings::{FormatSettings, PreviewMode};
    use ruff_source_file::LineIndex;

    use super::{format_text, minimal_edit};
    use crate::PositionEncoding;

    fn settings() -> FormatSettings {
        FormatSettings {
            preview: PreviewMode::Enabled,
            ..FormatSettings::default()
        }
    }

    const UNFORMATTED: &str = "program p
integer :: i
do i = 1, 3
print *, i
end do
end program p
";

    const FORMATTED: &str = "program p
  integer :: i
  do i = 1, 3
    print *, i
  end do
end program p
";

    #[test]
    fn formats_text() -> anyhow::Result<()> {
        assert_eq!(
            format_text(UNFORMATTED, &settings())?.as_deref(),
            Some(FORMATTED)
        );
        Ok(())
    }

    #[test]
    fn formatted_text_is_unchanged() -> anyhow::Result<()> {
        assert_eq!(format_text(FORMATTED, &settings())?, None);
        assert_eq!(format_text("", &settings())?, None);
        Ok(())
    }

    #[test]
    fn keeps_crlf_line_endings() -> anyhow::Result<()> {
        let crlf = |text: &str| text.replace('\n', "\r\n");
        assert_eq!(
            format_text(&crlf(UNFORMATTED), &settings())?,
            Some(crlf(FORMATTED))
        );
        assert_eq!(format_text(&crlf(FORMATTED), &settings())?, None);
        Ok(())
    }

    #[test]
    fn indent_width_from_settings() -> anyhow::Result<()> {
        let settings = FormatSettings {
            indent_width: 4,
            ..settings()
        };
        let formatted = format_text(UNFORMATTED, &settings)?.unwrap();
        assert!(formatted.contains("\n        print *, i\n"), "{formatted}");
        Ok(())
    }

    #[test]
    fn edit_only_covers_changed_lines() {
        let source = "program p
  integer :: i
  do i = 1, 3
print *, i
  end do
end program p
";
        let edit = minimal_edit(
            source,
            &LineIndex::from_source_text(source),
            FORMATTED,
            PositionEncoding::UTF16,
        );
        assert_eq!(edit.range.start, lsp_types::Position::new(3, 0));
        assert_eq!(edit.range.end, lsp_types::Position::new(4, 0));
        assert_eq!(edit.new_text, "    print *, i\n");
    }
}
