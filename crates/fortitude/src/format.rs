use std::{io::BufWriter, process::ExitCode};

use fortitude_linter::format::{create_formatter, format_file};
use fortitude_workspace::resolver::fortran_files_in_path;

use crate::cli::{FormatArgs, GlobalConfigArgs};
use crate::resolve;

use anyhow::Result;

/// Run the formatter over a whole project
///
/// TODO: Proper options like ``check``
pub fn format(args: FormatArgs, global_options: GlobalConfigArgs) -> Result<ExitCode> {
    let (files, config_arguments) = args.partition(global_options)?;
    let fortitude_config = resolve::resolve(&config_arguments, None)?;
    let (paths, _resolver) =
        fortran_files_in_path(&files, &None, &fortitude_config, &config_arguments)?;

    let language = create_formatter();

    for path in paths {
        let file = match path {
            Ok(resolved_file) => resolved_file.into_path(),
            Err(err) => {
                println!("Error finding files to format: {err}");
                return Ok(ExitCode::FAILURE);
            }
        };

        let output = std::io::stdout();
        let mut buf_output = BufWriter::new(output);

        match format_file(file, &language, &mut buf_output) {
            Ok(_) => continue,
            Err(err) => {
                println!("Formatter error: {err}");
                return Ok(ExitCode::FAILURE);
            }
        };
    }

    Ok(ExitCode::SUCCESS)
}
