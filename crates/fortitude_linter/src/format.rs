use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
};

use topiary_core::{FormatterError, Language, Operation, TopiaryQuery, formatter};

fn topiary_query() -> &'static str {
    include_str!("../resources/format/fortran.scm")
}

/// Create the topiary formatter
pub fn create_formatter() -> Language {
    let grammar: topiary_tree_sitter_facade::Language = tree_sitter_fortran::LANGUAGE.into();
    let query = TopiaryQuery::new(&grammar, topiary_query()).expect("building topiary query");
    Language {
        name: "fortran".to_string(),
        query,
        grammar,
        indent: None,
    }
}

/// Format an individual file, printing to stdout
pub fn format_file(file: PathBuf, language: &Language) -> Result<(), FormatterError> {
    println!("formatting {file:?}");
    let input = File::open(file)?;

    let output = std::io::stdout();
    let mut buf_input = BufReader::new(input);
    let mut buf_output = BufWriter::new(output);

    formatter(
        &mut buf_input,
        &mut buf_output,
        language,
        // TODO: user args?
        Operation::Format {
            skip_idempotence: true,
            tolerate_parsing_errors: true,
        },
    )?;

    buf_output.into_inner()?;
    Ok(())
}
