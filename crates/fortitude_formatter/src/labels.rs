//! Putting statement labels in the first column (the `col0-labels` setting).
//!
//! The label is moved to the first column and the statement after it stays
//! where it is indented, so `   10 continue` becomes `10 continue`, and with
//! the statement indented by six columns, `      8888 continue` becomes
//! `8888  continue`. A label at least as long as the indentation is followed
//! by a single blank.

use crate::range::continued_lines;

/// The number of columns of leading whitespace of `line`
fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

/// The length of the statement label at the start of `content`, a line
/// without its indentation, if there is one. A label has one to five digits
/// and is followed by a statement.
fn label_len(content: &str) -> Option<usize> {
    let digits = content.bytes().take_while(u8::is_ascii_digit).count();
    let rest = &content[digits..];
    ((1..=5).contains(&digits) && rest.starts_with([' ', '\t']) && !rest.trim().is_empty())
        .then_some(digits)
}

/// `line`, the first line of a statement, with its label in the first column
/// and the statement where the label was. `None` if it has no label.
pub(crate) fn label_to_column0(line: &str) -> Option<String> {
    let indent = indentation(line);
    let content = &line[indent..];
    let len = label_len(content)?;
    let blanks = indent.saturating_sub(len).max(1);
    Some(format!(
        "{}{}{}",
        &content[..len],
        " ".repeat(blanks),
        content[len..].trim_start()
    ))
}

/// The indentation of the statement on `line`, the first line of a
/// statement: the column after its label if the label is in the first column
pub(crate) fn statement_indentation(line: &str) -> usize {
    match label_len(line) {
        Some(len) => len + indentation(&line[len..]),
        None => indentation(line),
    }
}

/// Put the labels of the statements on `lines` in the first column
pub(crate) fn labels_to_column0(lines: &mut [String]) {
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let continued = continued_lines(&refs);
    for i in 0..lines.len() {
        if (i == 0 || !continued[i - 1])
            && let Some(line) = label_to_column0(&lines[i])
        {
            lines[i] = line;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{label_to_column0, labels_to_column0, statement_indentation};

    #[test]
    fn label_moved() {
        assert_eq!(
            label_to_column0("      8888 continue").as_deref(),
            Some("8888  continue")
        );
        assert_eq!(
            label_to_column0("  123456 x = 1").as_deref(),
            None,
            "not a label"
        );
        assert_eq!(
            label_to_column0("  12345 x = 1").as_deref(),
            Some("12345 x = 1")
        );
        assert_eq!(
            label_to_column0("10 continue").as_deref(),
            Some("10 continue")
        );
        assert_eq!(label_to_column0("  x = 1"), None);
        assert_eq!(label_to_column0("  10"), None);
    }

    #[test]
    fn continuation_lines_kept() {
        let mut lines: Vec<String> = [
            "   x = 1 + &",
            "      10 * y",
            "   10 y = 2 &",
            "      ! comment",
            "      + 20 &",
            "      20 + 3",
            "      30 z = 1",
        ]
        .map(String::from)
        .to_vec();
        labels_to_column0(&mut lines);
        assert_eq!(
            lines,
            [
                "   x = 1 + &",
                "      10 * y",
                "10 y = 2 &",
                "      ! comment",
                "      + 20 &",
                "      20 + 3",
                "30    z = 1",
            ]
        );
    }

    #[test]
    fn indentation_of_statements() {
        assert_eq!(statement_indentation("8888  continue"), 6);
        assert_eq!(statement_indentation("   continue"), 3);
        assert_eq!(statement_indentation("  10 continue"), 2);
    }
}
