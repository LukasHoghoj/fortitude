use std::{
    io::{BufWriter, Write},
    process::ExitCode,
};

use fortitude_formatter::{
    cached_formatter, format_file, format_file_in_place, format_text_or_lines,
};
use fortitude_linter::settings::PreviewMode;
use fortitude_workspace::resolver::fortran_files_in_path;

use crate::check::{is_stdin, resolve_default_files};
use crate::cli::{FormatCommand, GlobalConfigArgs};
use crate::resolve;
use crate::stdin::read_from_stdin;

use anyhow::Result;

/// Run the formatter over a whole project
///
/// TODO: Proper options like ``check``
pub fn format(args: FormatCommand, global_options: GlobalConfigArgs) -> Result<ExitCode> {
    let (cli, config_arguments) = args.partition(global_options)?;

    if !cli.i_understand_the_risks {
        println!(
            "The format command is still in development and may break your code (although it
_probably_ won't), and future changes may give different results. To use the
format command, you must set `--i-understand-the-risks` on the command line to acknowledge the
risks."
        );
        return Ok(ExitCode::FAILURE);
    }

    let is_stdin = is_stdin(&cli.files, cli.stdin_filename.as_deref());
    if is_stdin && cli.in_place {
        anyhow::bail!("--in-place can't be used when formatting standard input");
    }

    // Settings from the config file, overridden with options from the CLI
    let fortitude_config = resolve::resolve(&config_arguments, cli.stdin_filename.as_deref())?;
    let settings = &fortitude_config.settings;

    if settings.format.preview == PreviewMode::Disabled {
        println!("Format mode is currently in preview; nothing to do");
        return Ok(ExitCode::SUCCESS);
    }

    let language = cached_formatter(settings.format.indent_width);

    let lines: Vec<_> = cli.ranges.iter().map(|range| range.to_indices()).collect();
    let lines = (!lines.is_empty()).then_some(lines.as_slice());

    if is_stdin {
        let source = read_from_stdin()?;
        let mut stdout = std::io::stdout().lock();
        return match format_text_or_lines(&source, &language, &settings.format, lines) {
            Ok(formatted) => {
                stdout.write_all(formatted.as_bytes())?;
                Ok(ExitCode::SUCCESS)
            }
            Err(err) => {
                // Write the input back unchanged, in case it's replacing the
                // contents of an editor buffer
                stdout.write_all(source.as_bytes())?;
                let name = cli
                    .stdin_filename
                    .as_deref()
                    .unwrap_or(std::path::Path::new("-"));
                eprintln!("Formatter error in {}: {err}", name.display());
                Ok(ExitCode::FAILURE)
            }
        };
    }

    let files = resolve_default_files(cli.files, false);
    let (paths, _resolver) =
        fortran_files_in_path(&files, &None, &fortitude_config, &config_arguments)?;

    let mut changed = 0;
    let mut unchanged = 0;
    let mut failed = 0;

    let mut resolved_files = Vec::new();
    for path in paths {
        match path {
            Ok(resolved_file) => resolved_files.push(resolved_file.into_path()),
            Err(err) => {
                eprintln!("Error finding files to format: {err}");
                failed += 1;
            }
        }
    }

    let is_single_file = resolved_files.len() == 1 && files.iter().all(|path| !path.is_dir());
    let files = resolved_files;
    if lines.is_some() && !is_single_file {
        anyhow::bail!("--range can only be used when formatting a single file");
    }

    let mut stdout = BufWriter::new(std::io::stdout());

    for file in &files {
        let result = if cli.in_place {
            format_file_in_place(file, &language, &settings.format, lines).map(|was_changed| {
                if was_changed {
                    changed += 1;
                } else {
                    unchanged += 1;
                }
            })
        } else {
            // Tell the files apart if there are several of them
            if files.len() > 1 {
                writeln!(stdout, "formatting {file:?}")?;
            }
            format_file(file, &language, &settings.format, lines, &mut stdout)
        };

        if let Err(err) = result {
            stdout.flush()?;
            eprintln!("Formatter error in {}: {err}", file.display());
            failed += 1;
        }
    }
    stdout.flush()?;

    if cli.in_place {
        let plural = |n: usize| if n == 1 { "file" } else { "files" };
        let mut summary = vec![
            format!("{changed} {} reformatted", plural(changed)),
            format!("{unchanged} {} left unchanged", plural(unchanged)),
        ];
        if failed > 0 {
            summary.push(format!(
                "{failed} {} could not be formatted",
                plural(failed)
            ));
        }
        println!("{}", summary.join(", "));
    }

    if failed > 0 {
        Ok(ExitCode::FAILURE)
    } else {
        Ok(ExitCode::SUCCESS)
    }
}
