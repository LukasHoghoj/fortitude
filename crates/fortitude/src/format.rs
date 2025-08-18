use std::{io::BufWriter, process::ExitCode};

use fortitude_formatter::{create_formatter, format_file};
use fortitude_linter::settings::PreviewMode;
use fortitude_workspace::{configuration::resolve_bool_arg, resolver::fortran_files_in_path};

use crate::cli::{FormatCommand, GlobalConfigArgs};
use crate::resolve;

use anyhow::Result;

/// Run the formatter over a whole project
///
/// TODO: Proper options like ``check``
pub fn format(args: FormatCommand, global_options: GlobalConfigArgs) -> Result<ExitCode> {
    if !args.i_understand_the_risks.unwrap_or_default() {
        println!(
            "The format command is still in development and may break your code (although it
_probably_ won't), and future changes may give different results. To use the
format command, you must set `--i-understand-the-risks` on the command line to acknowledge the
risks."
        );
        return Ok(ExitCode::FAILURE);
    }

    let preview = resolve_bool_arg(args.preview, args.no_preview)
        .map(PreviewMode::from)
        .unwrap_or_default();

    if preview == PreviewMode::Disabled {
        println!("Format mode is currently in preview; nothing to do");
        return Ok(ExitCode::SUCCESS);
    }

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
