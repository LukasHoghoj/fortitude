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
        "program p\n  ! a comment that is far too long for\n  ! the line length we use here\nend program p\n"
    );
    Ok(())
}
