use crate::diagnostics::{AlwaysFixableViolation, Diagnostic, Edit, Fix};
use crate::{AstRule, CheckContext, kind_ids};
use fortitude_macros::ViolationMetadata;
use fortitude_sitter::Node;
use fortitude_sitter::traits::TextRanged;
use ruff_macros::derive_message_formats;
use ruff_text_size::{TextRange, TextSize};

/// ## What it does
/// Checks that the comment after an `#endif` matches the condition of the
/// `#if`, `#ifdef` or `#ifndef` that opens the block.
///
/// ## Why is this bad?
/// A comment after `#endif` helps to find the start of a long preprocessor
/// block. When the condition is renamed or changed and the comment is not, the
/// comment points to the wrong block.
///
/// When comparing, `defined(X)` and `defined X` are treated as `X`, and
/// whitespace is ignored. The block of an `#ifndef X` may be labelled either
/// `X` or `!X`. Comments that read as a description rather than a condition,
/// such as `/* end of MPI support */`, are not checked, and neither is an
/// `#endif` without a comment.
///
/// ## Example
/// ```f90
/// #ifdef USE_MPI
///   call mpi_init(ierr)
/// #endif /* USE_OPENMP */
/// ```
///
/// Use instead:
/// ```f90
/// #ifdef USE_MPI
///   call mpi_init(ierr)
/// #endif /* USE_MPI */
/// ```
///
/// ## Fix safety
/// This fix is unsafe, as the comment may be correct and the condition wrong.
#[derive(ViolationMetadata)]
pub(crate) struct MismatchedEndifComment {
    comment: String,
    condition: String,
}

impl AlwaysFixableViolation for MismatchedEndifComment {
    #[derive_message_formats]
    fn message(&self) -> String {
        let Self { comment, condition } = self;
        format!("`#endif` comment `{comment}` does not match the condition `{condition}`")
    }

    fn fix_title(&self) -> String {
        let Self { condition, .. } = self;
        format!("Replace the comment with `{condition}`")
    }
}

impl AstRule for MismatchedEndifComment {
    fn check(context: &CheckContext, node: &Node) -> Option<Vec<Diagnostic>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        // The comment is the token right after `#endif`, on the same line.
        let endif_index = children.iter().position(|child| child.kind() == "#endif")?;
        let endif = children[endif_index];
        let comment = children.get(endif_index + 1)?;
        if comment.start_position().row != endif.start_position().row {
            return None;
        }
        let (open, close) = match comment.kind() {
            "multiline_preproc_comment" => ("/*", "*/"),
            "inline_preproc_comment" => ("//", ""),
            _ => return None,
        };
        let comment_text = comment.text();
        let label = comment_text.strip_prefix(open)?.strip_suffix(close)?.trim();
        if label.is_empty() {
            return None;
        }

        // What the comment should say, and the canonical forms it may take.
        let (condition, accepted) = match children.first()?.kind() {
            "#if" => {
                let condition = collapse_whitespace(node.child_by_field_name("condition")?.text());
                let canonical = canonical_condition(&condition)?;
                let mut accepted = vec![canonical.clone()];
                // `#if !defined(X)` may be labelled `X`, like `#ifndef X`.
                if let Some(positive) = canonical.strip_prefix('!')
                    && is_identifier(positive)
                {
                    accepted.push(positive.to_string());
                }
                (condition, accepted)
            }
            "#ifdef" => {
                let name = node.child_by_field_name("name")?.text().to_string();
                (name.clone(), vec![name])
            }
            "#ifndef" => {
                let name = node.child_by_field_name("name")?.text().to_string();
                (name.clone(), vec![name.clone(), format!("!{name}")])
            }
            _ => return None,
        };

        // Comments that are prose rather than a condition are left alone.
        let canonical_label = canonical_condition(label)?;
        if accepted.contains(&canonical_label) {
            return None;
        }

        let replacement = if close.is_empty() {
            format!("{open} {condition}")
        } else {
            format!("{open} {condition} {close}")
        };
        // Inline comments run to the end of the line, so keep any carriage
        // return out of the range.
        let start = comment.start_textsize();
        let range = TextRange::at(
            start,
            TextSize::try_from(comment_text.trim_end().len()).ok()?,
        );
        let edit = Edit::range_replacement(replacement, range);

        some_vec![
            context
                .create_diagnostic(
                    Self {
                        comment: label.to_string(),
                        condition,
                    },
                    range,
                )
                .with_fix(Fix::unsafe_edit(edit))
        ]
    }

    fn entrypoints() -> Vec<u16> {
        kind_ids!["preproc_if", "preproc_ifdef"]
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.replace("\\\r\n", " ")
        .replace("\\\n", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Debug, PartialEq)]
enum Token<'a> {
    Word(&'a str),
    Punct(char),
}

/// Split a preprocessor condition into words and punctuation. Returns `None`
/// for text that can't be a condition, such as a comment written as prose.
fn tokenize(text: &str) -> Option<Vec<Token<'_>>> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            rest = &rest[c.len_utf8()..];
        } else if c.is_ascii_alphanumeric() || c == '_' {
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            tokens.push(Token::Word(&rest[..len]));
            rest = &rest[len..];
        } else if "()!&|<>=+-*/%^~".contains(c) {
            tokens.push(Token::Punct(c));
            rest = &rest[1..];
        } else {
            return None;
        }
    }
    Some(tokens)
}

/// Normalise a preprocessor condition so that equivalent spellings compare
/// equal: whitespace is removed, `defined(X)` and `defined X` become `X`, and
/// parentheses around the whole condition are dropped.
///
/// Returns `None` if the text doesn't look like a condition. In a condition,
/// two words are never next to each other, except after `defined`, so
/// `end of MPI support` is rejected.
fn canonical_condition(text: &str) -> Option<String> {
    let tokens = tokenize(text)?;
    let mut canonical = String::new();
    let mut previous_was_word = false;
    let mut i = 0;
    while i < tokens.len() {
        let word = match tokens[i] {
            Token::Punct(c) => {
                canonical.push(c);
                previous_was_word = false;
                i += 1;
                continue;
            }
            Token::Word("defined") => match tokens.get(i + 1..i + 4) {
                Some([Token::Punct('('), Token::Word(name), Token::Punct(')')]) => {
                    i += 4;
                    *name
                }
                _ => match tokens.get(i + 1) {
                    Some(Token::Word(name)) => {
                        i += 2;
                        *name
                    }
                    _ => return None,
                },
            },
            Token::Word(word) => {
                i += 1;
                word
            }
        };
        if previous_was_word {
            return None;
        }
        canonical.push_str(word);
        previous_was_word = true;
    }
    if canonical.is_empty() {
        return None;
    }
    while let Some(inner) = canonical
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .filter(|inner| parens_balanced(inner))
    {
        let inner = inner.to_string();
        canonical = inner;
    }
    Some(canonical)
}

fn parens_balanced(text: &str) -> bool {
    let mut depth = 0i32;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

#[cfg(test)]
mod tests {
    use super::canonical_condition;
    use test_case::test_case;

    #[test_case("FLAG", Some("FLAG"); "name")]
    #[test_case("  FLAG  ", Some("FLAG"); "name_whitespace")]
    #[test_case("defined(FLAG)", Some("FLAG"); "defined_parens")]
    #[test_case("defined ( FLAG )", Some("FLAG"); "defined_parens_spaces")]
    #[test_case("defined FLAG", Some("FLAG"); "defined_no_parens")]
    #[test_case("!defined(FLAG)", Some("!FLAG"); "not_defined")]
    #[test_case("defined(A) && !defined(B)", Some("A&&!B"); "and_not")]
    #[test_case("(A || B)", Some("A||B"); "outer_parens")]
    #[test_case("(A) || (B)", Some("(A)||(B)"); "inner_parens")]
    #[test_case("VERSION >= 3", Some("VERSION>=3"); "comparison")]
    #[test_case("end of MPI support", None; "prose")]
    #[test_case("defined A B", None; "two_words")]
    #[test_case("FLAG.", None; "invalid_character")]
    #[test_case("", None; "empty")]
    fn canonical(text: &str, expected: Option<&str>) {
        assert_eq!(canonical_condition(text).as_deref(), expected);
    }
}
