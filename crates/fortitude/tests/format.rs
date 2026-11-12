use assert_cmd::prelude::*;
use insta_cmd::assert_cmd_snapshot;
use std::{fs, process::Command};
use tempfile::TempDir;

const BIN_NAME: &str = "fortitude";

const UNFORMATTED: &str = "program p
integer :: i ! counter
do i = 1, 3
print *, i
end do
end program p
";

const FORMATTED: &str = "program p
  integer :: i ! counter
  do i = 1, 3
    print *, i
  end do
end program p
";

fn format_command(tempdir: &TempDir) -> anyhow::Result<Command> {
    let mut command = Command::cargo_bin(BIN_NAME)?;
    command
        .current_dir(tempdir.path())
        .arg("format")
        .arg("--preview")
        .arg("--i-understand-the-risks");
    Ok(command)
}

#[test]
fn format_to_stdout() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let test_file = tempdir.path().join("test.f90");
    fs::write(&test_file, UNFORMATTED)?;

    assert_cmd_snapshot!(format_command(&tempdir)?.arg("test.f90"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    program p
      integer :: i ! counter
      do i = 1, 3
        print *, i
      end do
    end program p

    ----- stderr -----
    ");
    // The file itself is left alone
    assert_eq!(fs::read_to_string(&test_file)?, UNFORMATTED);
    Ok(())
}

#[test]
fn format_in_place() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let test_file = tempdir.path().join("test.f90");
    fs::write(&test_file, UNFORMATTED)?;

    assert_cmd_snapshot!(format_command(&tempdir)?.arg("--in-place").arg("test.f90"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    1 file reformatted, 0 files left unchanged

    ----- stderr -----
    ");
    assert_eq!(fs::read_to_string(&test_file)?, FORMATTED);

    // Formatting again doesn't change the file, nor write it
    let modified = fs::metadata(&test_file)?.modified()?;
    assert_cmd_snapshot!(format_command(&tempdir)?.arg("--in-place").arg("test.f90"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    0 files reformatted, 1 file left unchanged

    ----- stderr -----
    ");
    assert_eq!(fs::read_to_string(&test_file)?, FORMATTED);
    assert_eq!(fs::metadata(&test_file)?.modified()?, modified);
    Ok(())
}

#[test]
fn format_directory_in_place() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let unformatted = tempdir.path().join("unformatted.f90");
    let formatted = tempdir.path().join("formatted.f90");
    let long = tempdir.path().join("long.f90");
    fs::write(&unformatted, UNFORMATTED)?;
    fs::write(&formatted, FORMATTED)?;
    fs::write(
        &long,
        "program p\n! a comment that is far too long for the line length we use here\nend program p\n",
    )?;

    assert_cmd_snapshot!(format_command(&tempdir)?
                         .arg("--in-place")
                         .arg("--split-comments")
                         .arg("--line-length=40")
                         .arg("."), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    2 files reformatted, 1 file left unchanged

    ----- stderr -----
    ");
    assert_eq!(fs::read_to_string(&unformatted)?, FORMATTED);
    assert_eq!(fs::read_to_string(&formatted)?, FORMATTED);
    assert_eq!(
        fs::read_to_string(&long)?,
        "program p\n! a comment that is far too long for the\n! line length we use here\nend program p\n"
    );
    Ok(())
}

#[test]
fn format_with_indent() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let test_file = tempdir.path().join("test.f90");
    fs::write(&test_file, UNFORMATTED)?;

    assert_cmd_snapshot!(format_command(&tempdir)?.arg("--indent=4").arg("test.f90"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    program p
        integer :: i ! counter
        do i = 1, 3
            print *, i
        end do
    end program p

    ----- stderr -----
    ");
    Ok(())
}

#[test]
fn format_column0_comments() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let test_file = tempdir.path().join("test.f90");
    fs::write(&test_file, "program p\n! column 0\nx = 1\nend program p\n")?;

    assert_cmd_snapshot!(format_command(&tempdir)?.arg("test.f90"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    program p
    ! column 0
      x = 1
    end program p

    ----- stderr -----
    ");
    assert_cmd_snapshot!(format_command(&tempdir)?.arg("--indent-col0-comments").arg("test.f90"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    program p
      ! column 0
      x = 1
    end program p

    ----- stderr -----
    ");
    Ok(())
}

/// Format `UNFORMATTED` in a directory with the given `fortitude.toml`, without
/// passing `--preview` on the command line
fn format_with_config(config: &str, args: &[&str]) -> anyhow::Result<String> {
    let tempdir = TempDir::new()?;
    fs::write(tempdir.path().join("fortitude.toml"), config)?;
    fs::write(tempdir.path().join("test.f90"), UNFORMATTED)?;
    let output = Command::cargo_bin(BIN_NAME)?
        .current_dir(tempdir.path())
        .arg("format")
        .arg("--i-understand-the-risks")
        .args(args)
        .arg("test.f90")
        .output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn format_settings_from_config_file() -> anyhow::Result<()> {
    let config = "
[format]
preview = true
indent-width = 4
";
    let stdout = format_with_config(config, &[])?;
    assert_eq!(
        stdout,
        "program p
    integer :: i ! counter
    do i = 1, 3
        print *, i
    end do
end program p
"
    );
    Ok(())
}

#[test]
fn format_preview_disabled_in_config_file() -> anyhow::Result<()> {
    // `format.preview` takes precedence over `check.preview`
    let config = "
[check]
preview = true
[format]
preview = false
";
    let stdout = format_with_config(config, &[])?;
    assert_eq!(
        stdout,
        "Format mode is currently in preview; nothing to do\n"
    );

    // But the command line takes precedence over the config file
    let stdout = format_with_config(config, &["--preview"])?;
    assert_eq!(stdout, FORMATTED);
    Ok(())
}

#[test]
fn format_falls_back_to_check_settings() -> anyhow::Result<()> {
    let config = "
[check]
preview = true
indent-width = 3
";
    let stdout = format_with_config(config, &[])?;
    assert_eq!(
        stdout,
        "program p
   integer :: i ! counter
   do i = 1, 3
      print *, i
   end do
end program p
"
    );
    Ok(())
}

#[test]
fn format_command_line_overrides_config_file() -> anyhow::Result<()> {
    let config = "
[format]
preview = true
indent-width = 4
";
    let stdout = format_with_config(config, &["--indent", "2"])?;
    assert_eq!(stdout, FORMATTED);
    Ok(())
}

#[test]
fn format_line_length_from_config_file() -> anyhow::Result<()> {
    let config = "
[check]
line-length = 132
[format]
preview = true
line-length = 30
";
    let tempdir = TempDir::new()?;
    fs::write(tempdir.path().join("fortitude.toml"), config)?;
    fs::write(
        tempdir.path().join("test.f90"),
        "program p
print *, 'first', 'second', 'third', 'fourth'
end program p
",
    )?;
    let output = Command::cargo_bin(BIN_NAME)?
        .current_dir(tempdir.path())
        .args(["format", "--i-understand-the-risks", "test.f90"])
        .output()?;
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("'fourth'"), "{stdout}");
    assert!(
        stdout.lines().all(|line| line.len() <= 30),
        "lines longer than 30 characters:\n{stdout}"
    );
    Ok(())
}

const RANGE_SOURCE: &str = "program p
integer :: i
do i = 1, 3
print *, i
end do
x=1
end program p
";

#[test]
fn format_range_to_stdout() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let test_file = tempdir.path().join("test.f90");
    fs::write(&test_file, RANGE_SOURCE)?;

    // Only line 4 is formatted, but indented according to the code around it
    assert_cmd_snapshot!(format_command(&tempdir)?.args(["--range", "4", "test.f90"]), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    program p
    integer :: i
    do i = 1, 3
        print *, i
    end do
    x=1
    end program p

    ----- stderr -----
    ");
    Ok(())
}

#[test]
fn format_range_in_place() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let test_file = tempdir.path().join("test.f90");
    fs::write(&test_file, RANGE_SOURCE)?;

    assert_cmd_snapshot!(format_command(&tempdir)?.args(["--in-place", "--range", "5-6", "test.f90"]), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    1 file reformatted, 0 files left unchanged

    ----- stderr -----
    ");
    assert_eq!(
        fs::read_to_string(&test_file)?,
        "program p
integer :: i
do i = 1, 3
print *, i
  end do
  x = 1
end program p
"
    );
    Ok(())
}

#[test]
fn format_range_needs_single_file() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    fs::write(tempdir.path().join("a.f90"), RANGE_SOURCE)?;
    fs::write(tempdir.path().join("b.f90"), RANGE_SOURCE)?;

    assert_cmd_snapshot!(format_command(&tempdir)?.args(["--range", "1-2", "a.f90", "b.f90"]), @r"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    fortitude failed
    Error: --range can only be used when formatting a single file
    ");
    Ok(())
}

#[test]
fn format_range_invalid() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    assert_cmd_snapshot!(format_command(&tempdir)?.args(["--range", "3-2", "a.f90"]), @r"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: invalid value '3-2' for '--range <START-END>': the range ends (line 2) before it starts (line 3)

    For more information, try '--help'.
    ");
    Ok(())
}

#[test]
fn format_in_place_keeps_crlf() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let test_file = tempdir.path().join("test.f90");
    fs::write(&test_file, UNFORMATTED.replace('\n', "\r\n"))?;

    format_command(&tempdir)?
        .args(["--in-place", "test.f90"])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(&test_file)?,
        FORMATTED.replace('\n', "\r\n")
    );
    Ok(())
}
