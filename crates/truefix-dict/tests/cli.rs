//! T068 (US13) — `truefix-dict` CLI subcommands (FR-018, SC-012). Drives the real compiled
//! binary as a subprocess (not the library functions directly), proving the CLI itself — argument
//! parsing, exit codes, error messages — works end-to-end, not just the logic it wraps.
#![cfg(feature = "dict-tooling")]

use std::path::PathBuf;
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_truefix-dict"))
}

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("truefix-dict-cli-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().expect("spawn truefix-dict")
}

#[test]
fn validate_reports_ok_for_a_bundled_dictionary() {
    let out = run(&["validate", "--dict", "dict-src/normalized/FIX44.fixdict"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("OK"), "stdout: {stdout}");
    assert!(stdout.contains("version=FIX.4.4"), "stdout: {stdout}");
    assert!(stdout.contains("hash="), "stdout: {stdout}");
}

#[test]
fn validate_reports_a_typed_error_for_a_malformed_dictionary() {
    let dir = scratch_dir();
    let bad = dir.join("bad.fixdict");
    std::fs::write(&bad, "not a valid dictionary\n").unwrap();
    let out = run(&["validate", "--dict", bad.to_str().unwrap()]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("error:"), "stderr: {stderr}");
}

#[test]
fn generate_dict_converts_the_bundled_orchestra_fixture() {
    let dir = scratch_dir();
    let out_path = dir.join("generated.fixdict");
    let out = run(&[
        "generate-dict",
        "--source",
        "dict-src/orchestra/FIXLATEST.orchestra.xml",
        "--out",
        out_path.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let generated = std::fs::read_to_string(&out_path).unwrap();
    let shipped = std::fs::read_to_string("dict-src/normalized/FIXLATEST.fixdict").unwrap();
    assert_eq!(
        generated, shipped,
        "CLI output should match the shipped, build.rs-generated file"
    );
}

#[test]
fn generate_code_produces_typed_rust_from_a_sample_fixdict() {
    let dir = scratch_dir();
    let out_path = dir.join("generated.rs");
    let out = run(&[
        "generate-code",
        "--dict",
        "dict-src/normalized/FIX44.fixdict",
        "--out",
        out_path.to_str().unwrap(),
        "--name",
        "FIX44",
    ]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let code = std::fs::read_to_string(&out_path).unwrap();
    assert!(code.contains("FIX44_DICT_HASH"));
    assert!(code.contains("pub mod fix44_msgs"));
    assert!(code.contains("pub fn crack_fix44"));
}

#[test]
fn generate_code_default_module_name_derives_from_the_dictionary_version() {
    let dir = scratch_dir();
    let out_path = dir.join("generated.rs");
    let out = run(&[
        "generate-code",
        "--dict",
        "dict-src/normalized/FIX44.fixdict",
        "--out",
        out_path.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // version "FIX.4.4" -> alphanumeric-only, uppercased -> "FIX44"
    assert!(stdout.contains("FIX44"), "stdout: {stdout}");
}

#[test]
fn no_subcommand_prints_usage_and_exits_non_zero() {
    let out = run(&[]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage:"), "stderr: {stderr}");
}

#[test]
fn unknown_subcommand_is_a_clean_error_not_a_panic() {
    let out = run(&["bogus-subcommand"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown subcommand"), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
}

#[test]
fn missing_required_flag_is_a_clean_error_not_a_panic() {
    let out = run(&["validate"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--dict is required"), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
}
