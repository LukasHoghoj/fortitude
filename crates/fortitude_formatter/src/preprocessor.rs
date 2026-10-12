//! Handling of preprocessor directives (C preprocessor and fypp).
//!
//! The Fortran parser can't make sense of preprocessor directives, and the
//! branches of a conditional often contain alternative versions of the same
//! statement, for example different argument lists of a subroutine:
//!
//! ```fortran
//! #ifdef USE_A
//! subroutine foo(a, b)
//! #else
//! subroutine foo(a, b, c)
//! #endif
//! ```
//!
//! Formatting such a file in one go would open a new scope for each
//! branch. Instead, directives are replaced by marker comments and the source
//! is formatted once for each alternative branch of a conditional
//! (`#if`/`#ifdef`/`#ifndef` ... `#elif`/`#else` ... `#endif`), with the other
//! branches left out. Every branch thus starts from the scope at the opening
//! `#if`, and the code after `#endif` is formatted as following the first
//! branch. The formatted pieces between the markers are then put together,
//! and the directives restored unchanged.
//!
//! Statements continued over lines containing directives are kept as they
//! are.

use std::collections::HashMap;

use crate::continuation::scan_line;

const MARKER: &str = "!fortitude-format-directive:";

fn marker(id: usize) -> String {
    format!("{MARKER}{id}")
}

/// Split a line at a marker, which the formatter may have put behind code
fn split_marker(line: &str) -> Option<(&str, usize)> {
    let (code, id) = line.split_once(MARKER)?;
    Some((code.trim_end(), id.trim().parse().ok()?))
}

/// Kind of a preprocessor directive line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Directive {
    If,
    Else,
    EndIf,
    Other,
}

/// Classify `line` if it is a preprocessor directive (C preprocessor or fypp)
fn directive_kind(line: &str) -> Option<Directive> {
    let trimmed = line.trim_start();
    // fypp inline eval and direct call directives
    if trimmed.starts_with("$:") || trimmed.starts_with("@:") {
        return Some(Directive::Other);
    }
    let rest = trimmed.strip_prefix('#')?;
    // fypp control directives start with '#:'
    let rest = rest.strip_prefix(':').unwrap_or(rest).trim_start();
    let keyword: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_lowercase();
    Some(match keyword.as_str() {
        "if" | "ifdef" | "ifndef" => Directive::If,
        "elif" | "else" => Directive::Else,
        "endif" => Directive::EndIf,
        _ => Directive::Other,
    })
}

/// A directive line is continued with a trailing '\' (C preprocessor) or '&' (fypp)
fn directive_continues(line: &str) -> bool {
    let line = line.trim_end();
    line.ends_with('\\') || line.ends_with('&')
}

#[derive(Debug)]
enum Item {
    /// A line of Fortran code
    Line(usize),
    /// A directive, or a statement kept as it is, replaced by a marker
    Marker(usize),
    /// A conditional, index into `Source::conds`
    Cond(usize),
}

#[derive(Debug, Default)]
struct Cond {
    /// Markers of the directives opening each branch
    openers: Vec<usize>,
    branches: Vec<Vec<Item>>,
    /// Marker of the closing `#endif`
    end: Option<usize>,
}

#[derive(Debug, Default)]
struct Source<'a> {
    lines: Vec<&'a str>,
    /// Original lines replaced by each marker
    markers: Vec<Vec<&'a str>>,
    /// Markers standing for statements kept as they are, rather than directives
    verbatim: Vec<bool>,
    conds: Vec<Cond>,
    items: Vec<Item>,
}

/// Classification of each line of the source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Code,
    Directive(Directive),
    /// Continuation of the previous directive line
    DirectiveContinuation,
    /// Part of a statement continued over directive lines
    Verbatim,
}

fn classify_lines(lines: &[&str]) -> Vec<LineKind> {
    let mut kinds = vec![LineKind::Code; lines.len()];

    let mut in_string = None;
    let mut continued = false;
    let mut directive_continued = false;
    let mut statement_start = 0;
    let mut directive_in_statement = false;

    for (i, line) in lines.iter().enumerate() {
        if directive_continued {
            kinds[i] = LineKind::DirectiveContinuation;
            directive_continued = directive_continues(line);
            directive_in_statement |= continued;
            continue;
        }
        if in_string.is_none() {
            if let Some(kind) = directive_kind(line) {
                kinds[i] = LineKind::Directive(kind);
                directive_continued = directive_continues(line);
                directive_in_statement |= continued;
                continue;
            }
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('!') {
                continue;
            }
        }

        if !continued {
            statement_start = i;
        }
        let scan = scan_line(line, in_string);
        in_string = scan.open_string;
        continued = in_string.is_some() || line[..scan.code_end].trim_end().ends_with('&');

        if !continued {
            if directive_in_statement {
                kinds[statement_start..=i].fill(LineKind::Verbatim);
            }
            directive_in_statement = false;
        }
    }

    kinds
}

impl<'a> Source<'a> {
    fn parse(source: &'a str) -> Self {
        let lines: Vec<&str> = source.lines().collect();
        let kinds = classify_lines(&lines);

        let mut result = Source {
            lines,
            ..Default::default()
        };
        // Conditionals currently open
        let mut stack: Vec<usize> = vec![];

        let mut i = 0;
        while i < result.lines.len() {
            let kind = kinds[i];
            // Lines belonging together
            let mut end = i + 1;
            match kind {
                LineKind::Directive(_) => {
                    while kinds.get(end) == Some(&LineKind::DirectiveContinuation) {
                        end += 1;
                    }
                }
                LineKind::Verbatim => {
                    while kinds.get(end) == Some(&LineKind::Verbatim) {
                        end += 1;
                    }
                }
                _ => {}
            }

            let item = match kind {
                LineKind::Code | LineKind::DirectiveContinuation => Item::Line(i),
                _ => {
                    let id = result.markers.len();
                    result.markers.push(result.lines[i..end].to_vec());
                    result.verbatim.push(kind == LineKind::Verbatim);
                    match (kind, stack.last()) {
                        (LineKind::Directive(Directive::If), _) => {
                            let cond = result.conds.len();
                            result.conds.push(Cond {
                                openers: vec![id],
                                branches: vec![vec![]],
                                end: None,
                            });
                            result.items_of(&stack).push(Item::Cond(cond));
                            stack.push(cond);
                            i = end;
                            continue;
                        }
                        (LineKind::Directive(Directive::Else), Some(&cond)) => {
                            result.conds[cond].openers.push(id);
                            result.conds[cond].branches.push(vec![]);
                            i = end;
                            continue;
                        }
                        (LineKind::Directive(Directive::EndIf), Some(&cond)) => {
                            result.conds[cond].end = Some(id);
                            stack.pop();
                            i = end;
                            continue;
                        }
                        // Unbalanced '#else'/'#endif' are treated like other directives
                        _ => Item::Marker(id),
                    }
                }
            };
            result.items_of(&stack).push(item);
            i = end;
        }

        result
    }

    /// The list of items new items are added to
    fn items_of(&mut self, stack: &[usize]) -> &mut Vec<Item> {
        match stack.last() {
            Some(&cond) => self.conds[cond].branches.last_mut().unwrap(),
            None => &mut self.items,
        }
    }

    /// Source code with the given branch of each conditional
    fn variant(&self, selection: &[usize]) -> String {
        let mut out = vec![];
        self.variant_items(&self.items, selection, &mut out);
        out.join("\n")
    }

    fn variant_items(&self, items: &[Item], selection: &[usize], out: &mut Vec<String>) {
        for item in items {
            match item {
                Item::Line(i) => out.push(self.lines[*i].to_string()),
                Item::Marker(id) => out.push(marker(*id)),
                Item::Cond(c) => {
                    let cond = &self.conds[*c];
                    for (branch, (opener, items)) in
                        cond.openers.iter().zip(&cond.branches).enumerate()
                    {
                        out.push(marker(*opener));
                        if branch == selection[*c] {
                            self.variant_items(items, selection, out);
                        }
                    }
                    if let Some(end) = cond.end {
                        out.push(marker(end));
                    }
                }
            }
        }
    }
}

/// Formatted code of a variant, split at the markers
struct FormattedVariant {
    /// Lines before the first marker
    head: Vec<String>,
    /// Lines following each marker, up to the next one
    segments: HashMap<usize, Vec<String>>,
    /// Indentation the formatter gave each marker
    indents: HashMap<usize, usize>,
}

impl FormattedVariant {
    fn new(lines: Vec<String>) -> Self {
        let mut head = vec![];
        let mut segments: HashMap<usize, Vec<String>> = HashMap::new();
        let mut indents = HashMap::new();
        let mut current: Option<usize> = None;
        for line in lines {
            let (code, marker) = match split_marker(&line) {
                Some((code, id)) => (code, Some(id)),
                None => (line.as_str(), None),
            };
            if marker.is_none() || !code.trim().is_empty() {
                match current {
                    Some(id) => segments.get_mut(&id).unwrap().push(code.to_string()),
                    None => head.push(code.to_string()),
                }
            }
            if let Some(id) = marker {
                segments.insert(id, vec![]);
                indents.insert(id, line.len() - line.trim_start().len());
                current = Some(id);
            }
        }
        Self {
            head,
            segments,
            indents,
        }
    }
}

/// Formats code in which directives are replaced by markers
struct Splicer<'s, 'a, F> {
    source: &'s Source<'a>,
    format: F,
    variants: HashMap<Vec<usize>, FormattedVariant>,
}

impl<F, E> Splicer<'_, '_, F>
where
    F: FnMut(&str) -> Result<Vec<String>, E>,
{
    fn variant(&mut self, selection: &[usize]) -> Result<&FormattedVariant, E> {
        if !self.variants.contains_key(selection) {
            let lines = (self.format)(&self.source.variant(selection))?;
            self.variants
                .insert(selection.to_vec(), FormattedVariant::new(lines));
        }
        Ok(&self.variants[selection])
    }

    /// Add a marker's original lines followed by the formatted code after it
    /// in the variant `selection`. Returns false if the marker got lost.
    fn marker(&mut self, id: usize, selection: &[usize], out: &mut Vec<String>) -> Result<bool, E> {
        let source = self.source;
        let variant = self.variant(selection)?;
        let (Some(segment), Some(&indent)) = (variant.segments.get(&id), variant.indents.get(&id))
        else {
            return Ok(false);
        };
        let original = source.markers[id].iter().map(|l| l.trim_end());
        if source.verbatim[id] {
            out.extend(shift_indent(original, indent));
        } else {
            out.extend(original.map(String::from));
        }
        out.extend(segment.iter().cloned());
        Ok(true)
    }

    fn items(
        &mut self,
        items: &[Item],
        selection: &mut Vec<usize>,
        out: &mut Vec<String>,
    ) -> Result<bool, E> {
        let source = self.source;
        for item in items {
            match item {
                Item::Line(_) => {}
                Item::Marker(id) => {
                    if !self.marker(*id, selection, out)? {
                        return Ok(false);
                    }
                }
                Item::Cond(c) => {
                    let cond = &source.conds[*c];
                    for (branch, (opener, items)) in
                        cond.openers.iter().zip(&cond.branches).enumerate()
                    {
                        let previous = std::mem::replace(&mut selection[*c], branch);
                        let ok = self.marker(*opener, selection, out)?
                            && self.items(items, selection, out)?;
                        selection[*c] = previous;
                        if !ok {
                            return Ok(false);
                        }
                    }
                    if let Some(end) = cond.end {
                        if !self.marker(end, selection, out)? {
                            return Ok(false);
                        }
                    }
                }
            }
        }
        Ok(true)
    }
}

/// Indent `lines` so that the first one starts at `indent`, keeping the
/// relative indentation of the others. Directives stay as they are.
fn shift_indent<'a>(lines: impl Iterator<Item = &'a str>, indent: usize) -> Vec<String> {
    let mut shift: Option<isize> = None;
    lines
        .map(move |line| {
            if line.is_empty() || directive_kind(line).is_some() {
                return line.to_string();
            }
            let current = line.len() - line.trim_start().len();
            let shift = *shift.get_or_insert(indent as isize - current as isize);
            let new_indent = (current as isize + shift).max(0) as usize;
            format!("{}{}", " ".repeat(new_indent), line.trim_start())
        })
        .collect()
}

/// Format `source` containing preprocessor directives, using `format` to
/// format Fortran code without directives (but with comments in their place).
/// If the source can't be put together again, it is returned unchanged.
pub(crate) fn format_with_directives<F, E>(source: &str, format: F) -> Result<Vec<String>, E>
where
    F: FnMut(&str) -> Result<Vec<String>, E>,
{
    let parsed = Source::parse(source);
    let mut splicer = Splicer {
        source: &parsed,
        format,
        variants: HashMap::new(),
    };

    if parsed.markers.is_empty() {
        return (splicer.format)(source);
    }

    let mut selection = vec![0; parsed.conds.len()];
    let mut out = splicer.variant(&selection)?.head.clone();
    if splicer.items(&parsed.items, &mut selection, &mut out)? {
        Ok(out)
    } else {
        log::warn!("Could not format code around preprocessor directives");
        Ok(source.lines().map(|l| l.trim_end().to_string()).collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn identity(source: &str) -> Result<Vec<String>, ()> {
        Ok(source.lines().map(String::from).collect())
    }

    #[test]
    fn variants() {
        let source = "a\n#ifdef X\nb\n#  elif Y\nc\n#else\nd\n#endif\ne\n";
        let parsed = Source::parse(source);
        assert_eq!(parsed.conds.len(), 1);
        assert_eq!(
            parsed.variant(&[0]),
            format!("a\n{0}0\nb\n{0}1\n{0}2\n{0}3\ne", MARKER)
        );
        assert_eq!(
            parsed.variant(&[2]),
            format!("a\n{0}0\n{0}1\n{0}2\nd\n{0}3\ne", MARKER)
        );
    }

    #[test]
    fn splice_identity() {
        let source =
            "a\n#ifdef X\nb\n#if Y\nc\n#else\nc2\n#endif\n#else\nd\n#define Z \\\n  1\n#endif\ne";
        assert_eq!(
            format_with_directives(source, identity).unwrap().join("\n"),
            source
        );
    }

    #[test]
    fn directive_in_continued_statement() {
        let source = "call foo(a, &\n#ifdef X\n  b, &\n#endif\n  c)\nx = 1";
        let parsed = Source::parse(source);
        assert!(parsed.conds.is_empty());
        assert_eq!(parsed.variant(&[]), format!("{MARKER}0\nx = 1"));
        assert_eq!(
            format_with_directives(source, identity).unwrap().join("\n"),
            source
        );
    }

    #[test]
    fn marker_behind_code() {
        let variant = FormattedVariant::new(vec![
            "a".to_string(),
            format!("  b {MARKER}0"),
            format!("  {MARKER}1"),
            "  c".to_string(),
        ]);
        assert_eq!(variant.head, ["a", "  b"]);
        assert_eq!(variant.segments[&0], Vec::<String>::new());
        assert_eq!(variant.segments[&1], ["  c"]);
        assert_eq!(variant.indents[&1], 2);
    }

    #[test]
    fn shift_verbatim_statement() {
        let lines = ["call foo(a, &", "#ifdef X", "  b, &", "#endif", "  c)"];
        assert_eq!(
            shift_indent(lines.into_iter(), 4),
            [
                "    call foo(a, &",
                "#ifdef X",
                "      b, &",
                "#endif",
                "      c)"
            ]
        );
    }

    #[test]
    fn fypp_directives() {
        assert_eq!(directive_kind("#:if defined('X')"), Some(Directive::If));
        assert_eq!(directive_kind("  #:else"), Some(Directive::Else));
        assert_eq!(directive_kind("#:endif"), Some(Directive::EndIf));
        assert_eq!(
            directive_kind("#:for i in range(3)"),
            Some(Directive::Other)
        );
        assert_eq!(directive_kind("$:x"), Some(Directive::Other));
        assert_eq!(directive_kind("# ifndef X"), Some(Directive::If));
        assert_eq!(directive_kind("x = 1"), None);
    }
}
