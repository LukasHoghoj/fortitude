mod comments;
mod continuation;
mod preprocessor;
mod split;

use std::{
    cmp::Ordering,
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Result;

use comments::{
    attach_trailing_comments, detach_trailing_comments, mark_column0_comments,
    unmark_column0_comments,
};
use continuation::{
    indent_string_continuations, join_continuation_lines, rejoin_statements, restore_line_breaks,
};
use fortitude_linter::settings::FormatSettings;
use preprocessor::format_with_directives;
use split::{split_at_blanks, split_long_comments, split_long_strings};
use topiary_core::{FormatterError, Language, Operation, TopiaryQuery, formatter};
use tree_sitter::{Point, Query, QueryCursor, StreamingIterator, Tree, TreeCursor};

fn topiary_query() -> &'static str {
    include_str!("../resources/format/fortran.scm")
}

fn wrap_query() -> &'static str {
    include_str!("../resources/format/wrap.scm")
}

/// Create the topiary formatter, indenting by `indent_width` spaces per level
pub fn create_formatter(indent_width: usize) -> Language {
    let grammar: topiary_tree_sitter_facade::Language = tree_sitter_fortran::LANGUAGE.into();
    let query = TopiaryQuery::new(&grammar, topiary_query()).expect("building topiary query");
    Language {
        name: "fortran".to_string(),
        query,
        grammar,
        indent: Some(" ".repeat(indent_width)),
    }
}

/// Number of spaces per level of indentation of the formatter
fn indent_width(language: &Language) -> usize {
    language.indent.as_ref().map_or(2, String::len)
}

// TODOs:
//  - Error handling?
//  - Tests?
//  - Ignore regions?
//  - Optimisations?
//      - Query and look up nodes to insert line breaks via descendant_for_point_range()?
//      - Implement TextProvider for Vec<String>?
//      - Reuse parse / parse tree

/// Format an individual file, writing the result to `output`
pub fn format_file(
    file: PathBuf,
    language: &Language,
    settings: &FormatSettings,
    output: &mut impl Write,
) -> Result<(), FormatterError> {
    let source = std::fs::read_to_string(&file)?;
    let lines = format_source(&source, language, settings)?;

    writeln!(output, "{}", lines.join("\n"))?;

    Ok(())
}

/// Format an individual file in place. The file is only written if
/// formatting changes it. Returns whether the file was changed.
pub fn format_file_in_place(
    file: &Path,
    language: &Language,
    settings: &FormatSettings,
) -> Result<bool, FormatterError> {
    let source = std::fs::read_to_string(file)?;
    let mut formatted = format_source(&source, language, settings)?.join("\n");
    formatted.push('\n');

    if formatted == source {
        return Ok(false);
    }
    std::fs::write(file, formatted)?;
    Ok(true)
}

/// Format Fortran source code, returning the formatted lines
pub fn format_source(
    source: &str,
    language: &Language,
    settings: &FormatSettings,
) -> Result<Vec<String>, FormatterError> {
    format_with_directives(source, |code| format_code(code, language, settings))
}

/// Format Fortran source code without preprocessor directives
fn format_code(
    source: &str,
    language: &Language,
    settings: &FormatSettings,
) -> Result<Vec<String>, FormatterError> {
    // Continued strings are joined to be split again where necessary
    let joined = join_continuation_lines(source, settings.split_strings);
    // Comment lines in the first column stay there
    let lines = if settings.indent_col0_comments {
        joined.lines
    } else {
        mark_column0_comments(joined.lines)
    };
    let (lines, comments) = detach_trailing_comments(lines);
    let format_input = lines.join("\n");
    let mut format_output = vec![];

    formatter(
        &mut format_input.as_bytes(),
        &mut format_output,
        language,
        // TODO: user args?
        Operation::Format {
            skip_idempotence: true,
            tolerate_parsing_errors: true,
        },
    )?;

    let format_result = String::from_utf8(format_output).unwrap();
    let lines: Vec<String> = format_result.lines().map(|x| x.to_string()).collect();

    let lines = unmark_column0_comments(lines);
    // Statements separated by ';' stay on one line
    let mut lines = rejoin_statements(lines);
    indent_string_continuations(&mut lines);
    // Continuation lines are indented by one level, like the formatter does
    let indent = indent_width(language);
    // Statements with comments on continued lines keep their line breaks
    let mut lines = restore_line_breaks(lines, &joined.kept, indent);
    break_lines(&mut lines, settings.line_length, indent);
    if settings.split_strings {
        split_long_strings(&mut lines, settings.line_length, indent);
    }
    // Lines that are still too long are broken at blanks
    split_at_blanks(&mut lines, settings.line_length, indent);
    // Comments making a line too long are put on a line of their own
    let mut lines = attach_trailing_comments(
        lines,
        &comments,
        settings.line_length,
        settings.split_comments,
    );
    if settings.split_comments {
        split_long_comments(&mut lines, settings.line_length);
    }

    Ok(lines)
}

// An actual computed line break
#[derive(Debug, Clone)]
struct Break {
    point: Point,
    next_indent: usize,
}

#[derive(Debug, Clone)]
struct BreakCollection {
    breaks: Vec<Break>,
    pattern_id: usize,
    /// Some of the breaks are at the end of a line already
    broken: bool,
}

// Repeately break lines until no long lines remain
fn break_lines(lines: &mut Vec<String>, line_length: usize, indent: usize) -> usize {
    let mut lines_changed = 0;
    for _ in 1..10 {
        lines_changed = break_lines_once(lines, line_length, indent);
        if lines_changed == 0 {
            break;
        };
    }
    lines_changed
}

fn get_indent(line: &String) -> usize {
    line.find(|c: char| !c.is_whitespace()).unwrap_or(0)
}

fn break_lines_once(lines: &mut Vec<String>, line_length: usize, indent: usize) -> usize {
    let language = tree_sitter_fortran::LANGUAGE;
    let mut parser = tree_sitter::Parser::new();

    let long_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, x)| x.len() > line_length)
        .map(|(i, _)| i)
        .collect();

    if long_lines.len() == 0 {
        return 0;
    }

    parser
        .set_language(&language.into())
        .expect("Error loading Fortran parser");

    let tree = parser
        .parse_with_options(
            &mut |_byte: usize, position: Point| -> &[u8] {
                let row = position.row as usize;
                let column = position.column as usize;
                if row < lines.len() {
                    if column < lines[row].as_bytes().len() {
                        &lines[row].as_bytes()[column..]
                    } else {
                        b"\n"
                    }
                } else {
                    &[]
                }
            },
            None,
            None,
        )
        .unwrap();

    let query = Query::new(&language.into(), wrap_query()).unwrap();

    let i_root = query.capture_index_for_name("root").unwrap();
    let i_break_after = query.capture_index_for_name("break-after").unwrap();
    let i_no_break = query.capture_index_for_name("no-break").unwrap();

    let mut cursor = QueryCursor::new();

    // I couldn't figure out of to get text_provider to work with Vec<String> as input
    let text = lines.join("\n");

    let matches = cursor.matches(&query, tree.root_node(), text.as_bytes());

    let mut possible_breaks = HashMap::<usize, BreakCollection>::new();
    // Byte ranges of the nodes with breaks, and of those not to break within
    let mut break_ranges = HashMap::<usize, std::ops::Range<usize>>::new();
    let mut no_break_ranges = vec![];

    matches.for_each(|x| {
        if let Some(qc) = x.captures.iter().find(|qc| qc.index == i_no_break) {
            no_break_ranges.push(qc.node.byte_range());
            return;
        }
        let mut break_node_id = None;
        let mut breaks = vec![];
        let mut broken = false;
        x.captures.iter().for_each(|qc| {
            let range = &qc.node.range();
            if qc.index == i_root {
                assert!(
                    break_node_id.replace(qc.node.id()) == None,
                    "Should only be one root node"
                );
                break_ranges.insert(qc.node.id(), qc.node.byte_range());
            } else if qc.index == i_break_after {
                let row = qc.node.start_position().row;
                let after = &lines[range.end_point.row][range.end_point.column..];
                // A line may end with '&' and a comment
                let after = after.trim_start();
                let after = after.strip_prefix('&').unwrap_or(after).trim_start();
                broken |= after.is_empty() || after.starts_with('!');
                breaks.push(Break {
                    point: range.end_point,
                    next_indent: get_indent(&lines[row]) + indent,
                })
            }
            // Can there be more than on match per capture?
        });

        assert!(breaks.len() > 0, "There should be some breaks found");
        let break_node_id = break_node_id.expect("There should be some breaks found");

        match possible_breaks.get_mut(&break_node_id) {
            None => {
                possible_breaks.insert(
                    break_node_id,
                    BreakCollection {
                        breaks: breaks,
                        pattern_id: x.pattern_index,
                        broken,
                    },
                );
            }
            Some(b) => {
                assert!(
                    b.pattern_id == x.pattern_index,
                    "We should only match each node with the same pattern?"
                );
                b.breaks.extend(breaks);
                b.broken |= broken;
            }
        };
    });

    possible_breaks.retain(|id, _| {
        let range = &break_ranges[id];
        !no_break_ranges
            .iter()
            .any(|no_break| no_break.start <= range.start && range.end <= no_break.end)
    });

    let mut breaks: Vec<Break> = get_breaks(&tree, &long_lines, &mut possible_breaks)
        .iter()
        .flat_map(|x| x.breaks.clone())
        .collect();

    breaks.sort_by_key(|x| (x.point.row, x.point.column));

    let mut num_breaks = 0;

    breaks.iter().rev().for_each(|br| {
        num_breaks += 1;

        let line = lines.get_mut(br.point.row).unwrap();
        let next_line = line.split_off(br.point.column);
        line.push_str(" &");

        lines.insert(
            br.point.row + 1,
            format!("{}{}", " ".repeat(br.next_indent), next_line.trim_start()),
        );
    });

    num_breaks
}

fn get_breaks(
    tree: &Tree,
    long_lines: &Vec<usize>,
    possible_breaks: &mut HashMap<usize, BreakCollection>,
) -> Vec<BreakCollection> {
    let mut cursor = tree.walk();
    let mut actual_breaks = vec![];

    get_breaks_dfs(
        &mut cursor,
        long_lines,
        possible_breaks,
        &mut actual_breaks,
        false,
    );

    actual_breaks
}

fn get_breaks_dfs(
    cursor: &mut TreeCursor,
    long_lines: &Vec<usize>,
    possible_breaks: &mut HashMap<usize, BreakCollection>,
    actual_breaks: &mut Vec<BreakCollection>,
    ancestor_is_breakable: bool,
) -> Option<BreakCollection> {
    let node = cursor.node();

    if !node_contains_long_line(node, long_lines) {
        // No reason to continue if subtree contains no long lines
        return None;
    }

    let mut breaks = possible_breaks.remove(&node.id());

    // A node that spans several lines (e.g. because of a continued string
    // literal) can be broken as long as it hasn't been broken already
    let this_is_breakable = breaks.as_ref().is_some_and(|b| !b.broken);

    if !this_is_breakable && ancestor_is_breakable {
        // No reason to continue if parent is breakable, and this
        // node has no breaks for parent to absorb
        return None;
    }

    // The following implements a DFS search, searching for the
    // outer-most breakable scope of the syntax tree.
    if cursor.goto_first_child() {
        loop {
            let child_breaks = get_breaks_dfs(
                cursor,
                long_lines,
                possible_breaks,
                actual_breaks,
                ancestor_is_breakable || this_is_breakable,
            );

            // We need to absorb similar breaks into this the parent node to deal with eg. math expressions.
            match (&mut breaks, &child_breaks) {
                (Some(br), Some(ch)) if br.pattern_id == ch.pattern_id => {
                    br.breaks.extend(ch.breaks.clone());
                }
                _ => {}
            }

            if !cursor.goto_next_sibling() {
                break;
            };
        }
        assert!(cursor.goto_parent());
    }

    if (!ancestor_is_breakable) && this_is_breakable {
        // If we get to here, then this node is the outer-most breakable syntax node,
        // and should therefore be line-broken.
        actual_breaks.push(breaks.take().unwrap());
    }

    breaks
}

fn node_contains_long_line(node: tree_sitter::Node<'_>, long_lines: &Vec<usize>) -> bool {
    let a = node.start_position().row;
    let b = node.end_position().row;

    long_lines
        .binary_search_by(|&i| {
            if a <= i && i <= b {
                Ordering::Equal
            } else if i < a {
                Ordering::Less
            } else if i > b {
                Ordering::Greater
            } else {
                panic!("Should be unreachable")
            }
        })
        .is_ok()
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use anyhow::Result;
    use fortitude_linter::settings::FormatSettings;
    use insta::assert_snapshot;
    use lazy_static::lazy_static;
    use test_case::test_case;
    use topiary_core::{FormatterError, Language};

    macro_rules! apply_common_filters {
        {} => {
            let mut settings = insta::Settings::clone_current();
            // Convert windows paths to Unix Paths.
            settings.add_filter(r"\\\\?([\w\d.])", "/$1");
            let _bound = settings.bind_to_scope();
        }
    }

    use super::{create_formatter, format_file, format_source};

    lazy_static! {
        pub static ref TEST_FORMATTER: Language = create_formatter(2);
    }

    #[test_case(Path::new("simple.f90"))]
    #[test_case(Path::new("block-construct.f90"))]
    #[test_case(Path::new("block-data-obsolescent.f90"))]
    #[test_case(Path::new("deferred-binding.f90"))]
    #[test_case(Path::new("derived-type-attributes.f90"))]
    #[test_case(Path::new("derived-type-private-type-bound-procedures.f90"))]
    #[test_case(Path::new("derived-type-procedure-pointer-components.f90"))]
    #[test_case(Path::new("derived-type-variable-declarations.f90"))]
    #[test_case(Path::new("enumeration-type.f90"))]
    #[test_case(Path::new("functions-complex.f90"))]
    #[test_case(Path::new("functions-simple.f90"))]
    #[test_case(Path::new("interface-abstract.f90"))]
    #[test_case(Path::new("interface-assignment.f90"))]
    #[test_case(Path::new("interface-explicit-2.f90"))]
    #[test_case(Path::new("interface-explicit.f90"))]
    #[test_case(Path::new("interface-generic.f90"))]
    #[test_case(Path::new("interface-operator.f90"))]
    #[test_case(Path::new("long-lines.f90"))]
    #[test_case(Path::new("module-definition-with-internal-module-procedures.f90"))]
    #[test_case(Path::new("procedure-as-argument.f90"))]
    #[test_case(Path::new("program.f90"))]
    #[test_case(Path::new("semicolon-in-interface.f90"))]
    #[test_case(Path::new("submodule-definition-simple.f90"))]
    #[test_case(Path::new("subroutine.f90"))]
    #[test_case(Path::new("use-operator-and-assignment.f90"))]
    #[test_case(Path::new("continued-strings.f90"))]
    #[test_case(Path::new("continued-string-after-semicolon.f90"))]
    #[test_case(Path::new("continuation-comments.f90"))]
    #[test_case(Path::new("preprocessor-alternatives.f90"))]
    #[test_case(Path::new("fypp-directives.f90"))]
    #[test_case(Path::new("statements.f90"))]
    fn format(path: &Path) -> Result<(), FormatterError> {
        let snapshot = format!("{}", path.to_string_lossy());

        let path = Path::new("./resources/test/fixtures/format").join(path);

        let mut buf = Vec::new();
        let settings = FormatSettings::default();
        format_file(path, &TEST_FORMATTER, &settings, &mut buf)?;
        apply_common_filters!();

        let string = String::from_utf8(buf)?;
        assert_snapshot!(snapshot, string);

        Ok(())
    }

    #[test_case(Path::new("long-lines.f90"))]
    fn format_shorter_lines(path: &Path) -> Result<(), FormatterError> {
        let snapshot = format!("shorter_{}", path.to_string_lossy());

        let path = Path::new("./resources/test/fixtures/format").join(path);

        let mut buf = Vec::new();
        let settings = FormatSettings {
            line_length: 20,
            ..FormatSettings::default()
        };
        format_file(path, &TEST_FORMATTER, &settings, &mut buf)?;
        apply_common_filters!();

        let string = String::from_utf8(buf)?;
        assert_snapshot!(snapshot, string);

        Ok(())
    }

    #[test_case(Path::new("split-comments.f90"), 64, true, false)]
    #[test_case(Path::new("split-comments.f90"), 64, false, false)]
    #[test_case(Path::new("split-strings.f90"), 60, false, true)]
    #[test_case(Path::new("split-strings.f90"), 60, false, false)]
    #[test_case(Path::new("code-after-continued-string.f90"), 60, false, false)]
    #[test_case(Path::new("long-names-with-brackets.f90"), 90, false, false)]
    #[test_case(Path::new("preprocessor-long-headers.f90"), 70, false, false)]
    #[test_case(Path::new("continued-strings.f90"), 100, true, true)]
    #[test_case(Path::new("statements.f90"), 100, false, false)]
    #[test_case(Path::new("continuation-comments.f90"), 100, false, false)]
    #[test_case(Path::new("continuation-comments.f90"), 40, false, false)]
    #[test_case(Path::new("line-length.f90"), 40, false, false)]
    #[test_case(Path::new("line-length.f90"), 40, true, true)]
    #[test_case(Path::new("line-length.f90"), 60, false, false)]
    #[test_case(Path::new("statements.f90"), 60, false, false)]
    #[test_case(Path::new("preprocessor-alternatives.f90"), 100, false, false)]
    fn format_with_options(
        path: &Path,
        line_length: usize,
        split_comments: bool,
        split_strings: bool,
    ) -> Result<(), FormatterError> {
        let mut snapshot = format!("{}_{line_length}", path.to_string_lossy());
        if split_comments {
            snapshot.push_str("_split-comments");
        }
        if split_strings {
            snapshot.push_str("_split-strings");
        }

        let path = Path::new("./resources/test/fixtures/format").join(path);
        let source = std::fs::read_to_string(path)?;
        let settings = FormatSettings {
            line_length,
            split_comments,
            split_strings,
            ..FormatSettings::default()
        };
        let formatted = format_source(&source, &TEST_FORMATTER, &settings)?.join("\n");
        apply_common_filters!();
        assert_snapshot!(snapshot, formatted);

        // Formatting again doesn't change anything
        let reformatted = format_source(&formatted, &TEST_FORMATTER, &settings)?.join("\n");
        assert_eq!(formatted, reformatted);

        Ok(())
    }

    #[test_case(Path::new("statements.f90"), 4, 60)]
    #[test_case(Path::new("continuation-comments.f90"), 4, 60)]
    #[test_case(Path::new("preprocessor-alternatives.f90"), 3, 60)]
    #[test_case(Path::new("line-length.f90"), 3, 40)]
    fn format_with_indent(
        path: &Path,
        indent_width: usize,
        line_length: usize,
    ) -> Result<(), FormatterError> {
        let snapshot = format!(
            "{}_indent-{indent_width}_{line_length}",
            path.to_string_lossy()
        );
        let path = Path::new("./resources/test/fixtures/format").join(path);
        let source = std::fs::read_to_string(path)?;
        let formatter = create_formatter(indent_width);
        let settings = FormatSettings {
            line_length,
            indent_width,
            ..FormatSettings::default()
        };
        let formatted = format_source(&source, &formatter, &settings)?.join("\n");
        apply_common_filters!();
        assert_snapshot!(snapshot, formatted);

        // Formatting again doesn't change anything
        let reformatted = format_source(&formatted, &formatter, &settings)?.join("\n");
        assert_eq!(formatted, reformatted);

        Ok(())
    }

    #[test_case(false)]
    #[test_case(true)]
    fn format_column0_comments(indent_col0_comments: bool) -> Result<(), FormatterError> {
        let snapshot = format!("comments-column0.f90_indent-col0-{indent_col0_comments}");
        let path = Path::new("./resources/test/fixtures/format/comments-column0.f90");
        let source = std::fs::read_to_string(path)?;
        let settings = FormatSettings {
            indent_col0_comments,
            ..FormatSettings::default()
        };
        let formatted = format_source(&source, &TEST_FORMATTER, &settings)?.join("\n");
        apply_common_filters!();
        assert_snapshot!(snapshot, formatted);

        // Formatting again doesn't change anything
        let reformatted = format_source(&formatted, &TEST_FORMATTER, &settings)?.join("\n");
        assert_eq!(formatted, reformatted);
        Ok(())
    }

    /// With strings and comments split, no line exceeds the line length
    #[test_case(40)]
    #[test_case(60)]
    #[test_case(80)]
    fn line_length_respected(line_length: usize) -> Result<(), FormatterError> {
        let path = Path::new("./resources/test/fixtures/format/line-length.f90");
        let source = std::fs::read_to_string(path)?;
        let settings = FormatSettings {
            line_length,
            split_comments: true,
            split_strings: true,
            ..FormatSettings::default()
        };
        let formatted = format_source(&source, &TEST_FORMATTER, &settings)?;
        let too_long: Vec<&String> = formatted
            .iter()
            .filter(|line| line.len() > line_length)
            .collect();
        assert!(too_long.is_empty(), "lines too long: {too_long:#?}");
        Ok(())
    }
}
