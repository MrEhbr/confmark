use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn confmark() -> Command {
    Command::cargo_bin("confmark").unwrap()
}

#[test]
fn test_help_command() {
    confmark()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("--from"));
}

#[test]
fn test_version_flag() {
    confmark()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.0.1"));
}

#[test]
fn test_missing_required_args() {
    confmark()
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_identical_formats_rejected() {
    confmark()
        .args(["--from", "md", "--to", "md"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("must differ"));
}

#[test]
fn test_converts_markdown_to_confluence_from_stdin() {
    confmark()
        .args(["--from", "md", "--to", "cf"])
        .write_stdin("# Hi")
        .assert()
        .success()
        .stdout("<h1>Hi</h1>");
}

#[test]
fn test_short_flags_parse() {
    confmark()
        .args(["-f", "md", "-t", "cf"])
        .write_stdin("# Hi")
        .assert()
        .success()
        .stdout("<h1>Hi</h1>");
}

#[test]
fn test_converts_confluence_to_markdown_from_stdin() {
    confmark()
        .args(["--from", "cf", "--to", "md"])
        .write_stdin("<h1>Hi</h1>")
        .assert()
        .success()
        .stdout("# Hi");
}

#[test]
fn test_reads_input_file() {
    let input = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("in.md");
    fs::write(&input, "# Title").unwrap();

    confmark()
        .args(["-f", "md", "-t", "cf"])
        .arg(&input)
        .assert()
        .success()
        .stdout("<h1>Title</h1>");
}

#[test]
fn test_writes_output_file() {
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("out.xml");

    confmark()
        .args(["-f", "md", "-t", "cf", "-o"])
        .arg(&output)
        .write_stdin("# Hi")
        .assert()
        .success()
        .stdout("");

    assert_eq!(fs::read_to_string(&output).unwrap(), "<h1>Hi</h1>");
}

#[test]
fn test_missing_input_file_errors() {
    let missing = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("does-not-exist.md");

    confmark()
        .args(["-f", "md", "-t", "cf"])
        .arg(&missing)
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to read"));
}
