use crate::CheckContext;
use crate::diagnostics::{Diagnostic, Edit, Fix, FixAvailability, Violation};
use fortitude_macros::ViolationMetadata;
use ruff_macros::derive_message_formats;
use ruff_source_file::UniversalNewlines;
use ruff_text_size::{TextRange, TextSize};

/// ## What it does
/// Checks for text after `#endif` or `#else` that isn't in a comment, such as
/// `#endif USE_MPI`.
///
/// ## Why is this bad?
/// The C standard doesn't allow anything after `#endif` or `#else` except a
/// comment. Most preprocessors accept it anyway, but GCC warns about it
/// (`-Wendif-labels`), and Fortitude's parser can't read it: the rest of the
/// file is reported as a syntax error, and fixes can't be applied to it.
///
/// Writing the label as a C comment keeps it for the reader, and every
/// preprocessor removes it. The fix uses `/* */` rather than `//`, as
/// preprocessors in traditional mode, such as gfortran's, don't treat `//` as
/// a comment: it's Fortran's string concatenation operator.
///
/// Unlike most fixes, this one is applied even though the file has syntax
/// errors, as it's what repairs them.
///
/// ## Example
/// ```f90
/// #ifdef USE_MPI
///   call mpi_init(ierr)
/// #endif USE_MPI
/// ```
///
/// Use instead:
/// ```f90
/// #ifdef USE_MPI
///   call mpi_init(ierr)
/// #endif /* USE_MPI */
/// ```
#[derive(ViolationMetadata)]
pub(crate) struct UncommentedPreprocessorLabel {
    directive: String,
    label: String,
}

impl Violation for UncommentedPreprocessorLabel {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let Self { directive, label } = self;
        format!("Text `{label}` after `#{directive}` is not in a comment")
    }

    fn fix_title(&self) -> Option<String> {
        let Self { label, .. } = self;
        Some(format!("Write as `/* {label} */`"))
    }
}

impl UncommentedPreprocessorLabel {
    pub fn check(context: &CheckContext) -> Vec<Diagnostic> {
        let mut violations = Vec::new();
        for line in context.source_text().universal_newlines() {
            let Some((directive, offset, label)) = find_label(line.as_str()) else {
                continue;
            };
            let start = line.start() + TextSize::try_from(offset).unwrap();
            let range = TextRange::at(start, TextSize::try_from(label.len()).unwrap());
            let mut diagnostic = context.create_diagnostic(
                Self {
                    directive: directive.to_string(),
                    label: label.to_string(),
                },
                range,
            );
            // A label containing `*/` would end the comment early.
            if !label.contains("*/") {
                let edit = Edit::range_replacement(format!("/* {label} */"), range);
                diagnostic = diagnostic.with_fix(Fix::safe_edit(edit));
            }
            violations.push(diagnostic);
        }
        violations
    }
}

/// For a line such as `#endif USE_MPI`, returns the directive (`endif`), the
/// byte offset of the label in the line, and the label (`USE_MPI`). Text
/// after the label that is already a comment is not part of the label.
fn find_label(line: &str) -> Option<(&'static str, usize, &str)> {
    let after_hash = line.trim_start().strip_prefix('#')?;
    let after_space = after_hash.trim_start_matches([' ', '\t']);
    let (directive, rest) = ["endif", "else"].into_iter().find_map(|directive| {
        let rest = after_space.strip_prefix(directive)?;
        // `#endif` must be followed by whitespace or a comment, so that a
        // directive such as `#elseif` isn't mistaken for `#else`.
        (rest.is_empty() || rest.starts_with([' ', '\t', '/'])).then_some((directive, rest))
    })?;

    let comment_start = [rest.find("/*"), rest.find("//")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    let label = rest[..comment_start].trim();
    if label.is_empty() {
        return None;
    }
    // `rest` is a suffix of `line`, and the label starts after its whitespace.
    let offset = line.len() - rest.trim_start().len();
    Some((directive, offset, label))
}

#[cfg(test)]
mod tests {
    use super::find_label;
    use test_case::test_case;

    #[test_case("#endif", None)]
    #[test_case("#endif   ", None)]
    #[test_case("#endif /* USE_MPI */", None)]
    #[test_case("#endif // USE_MPI", None)]
    #[test_case("#endif/* USE_MPI */", None)]
    #[test_case("#else", None)]
    #[test_case("#elif USE_MPI", None)]
    #[test_case("#elseif USE_MPI", None)]
    #[test_case("#endifUSE_MPI", None)]
    #[test_case("#if USE_MPI", None)]
    #[test_case("x = 1 ! #endif USE_MPI", None)]
    #[test_case("#endif USE_MPI", Some(("endif", 7, "USE_MPI")))]
    #[test_case("#endif\tUSE_MPI  ", Some(("endif", 7, "USE_MPI")))]
    #[test_case("  #  endif USE_MPI", Some(("endif", 11, "USE_MPI")))]
    #[test_case("#else USE_MPI", Some(("else", 6, "USE_MPI")))]
    #[test_case("#endif !USE_MPI && X", Some(("endif", 7, "!USE_MPI && X")))]
    #[test_case("#endif USE_MPI /* parallel */", Some(("endif", 7, "USE_MPI")))]
    fn label(line: &str, expected: Option<(&str, usize, &str)>) {
        assert_eq!(find_label(line), expected);
    }
}
