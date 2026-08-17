//! End-to-end tests of the actual `psfmt` binary.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn psfmt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_psfmt"))
}

fn run_stdin(args: &[&str], input: &str) -> (String, String, i32) {
    let mut child = psfmt()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn psfmt");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8(out.stdout).expect("stdout utf8"),
        String::from_utf8(out.stderr).expect("stderr utf8"),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn stdin_filter_formats() {
    let (stdout, stderr, code) = run_stdin(&[], "function foo {\n\"hello\"\n  }");
    assert_eq!(stdout, "function foo {\n    \"hello\"\n}");
    assert_eq!(stderr, "");
    assert_eq!(code, 0);
}

#[test]
fn stdout_contains_only_source() {
    let (stdout, _, code) = run_stdin(&[], "$x=1");
    assert_eq!(stdout, "$x = 1");
    assert_eq!(code, 0);
}

#[test]
fn malformed_input_passes_through_with_diagnostics() {
    let src = "function f {\n  'x'\n";
    let (stdout, stderr, code) = run_stdin(&["--stdin-filepath", "broken.ps1"], src);
    assert_eq!(stdout, src, "malformed input must pass through unchanged");
    assert!(stderr.contains("broken.ps1"), "stderr: {stderr}");
    assert!(stderr.contains("unbalanced"), "stderr: {stderr}");
    assert_eq!(code, 4);
}

#[test]
fn check_mode_reports_changes() {
    let (stdout, _, code) = run_stdin(&["--check"], "$x=1");
    assert_eq!(stdout, "", "--check must not print source");
    assert_eq!(code, 1);
    let (_, _, code) = run_stdin(&["--check"], "$x = 1");
    assert_eq!(code, 0);
}

#[test]
fn preset_allman() {
    let (stdout, _, code) = run_stdin(&["--preset", "allman"], "if ($x) {\n1\n}");
    assert_eq!(stdout, "if ($x)\n{\n    1\n}");
    assert_eq!(code, 0);
}

#[test]
fn range_option() {
    let (stdout, _, code) = run_stdin(&["--range", "2,1,2,12"], "if($a){'x'}\nif($b){'y'}");
    assert_eq!(stdout, "if($a){'x'}\nif ($b) { 'y' }");
    assert_eq!(code, 0);
}

#[test]
fn config_file_applies() {
    let dir = std::env::temp_dir().join(format!("psfmt-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("cfg.json");
    std::fs::write(&cfg, r#"{"indentWidth": 2}"#).unwrap();
    let (stdout, _, code) = run_stdin(&["--config", cfg.to_str().unwrap()], "if ($x) {\n1\n}");
    assert_eq!(stdout, "if ($x) {\n  1\n}");
    assert_eq!(code, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_mode_is_atomic_and_check_after_write_passes() {
    let dir = std::env::temp_dir().join(format!("psfmt-w-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file: PathBuf = dir.join("script.ps1");
    std::fs::write(&file, "if($x){\n1\n}").unwrap();

    let status = psfmt()
        .args(["--write", file.to_str().unwrap()])
        .status()
        .expect("run");
    assert!(status.success());
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(text, "if ($x) {\n    1\n}");

    let status = psfmt()
        .args(["--check", file.to_str().unwrap()])
        .status()
        .expect("run");
    assert!(status.success(), "already-formatted file must pass --check");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_flag_is_usage_error() {
    let (_, stderr, code) = run_stdin(&["--frobnicate"], "");
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown option"));
}

#[test]
fn version_and_help() {
    let (stdout, _, code) = run_stdin(&["--version"], "");
    assert!(stdout.starts_with("psfmt "));
    assert_eq!(code, 0);
    let (stdout, _, code) = run_stdin(&["--help"], "");
    assert!(stdout.contains("USAGE"));
    assert_eq!(code, 0);
}

#[test]
fn catalog_enables_command_casing() {
    let dir = std::env::temp_dir().join(format!("psfmt-cat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cat = dir.join("catalog.json");
    std::fs::write(
        &cat,
        r#"{ "commands": { "Get-ChildItem": ["Path", "Recurse"] } }"#,
    )
    .unwrap();
    let (stdout, _, code) = run_stdin(
        &["--catalog", cat.to_str().unwrap()],
        "get-childitem -path C:\\ -recurse",
    );
    assert_eq!(stdout, "Get-ChildItem -Path C:\\ -Recurse");
    assert_eq!(code, 0);
    let _ = std::fs::remove_dir_all(&dir);
}
