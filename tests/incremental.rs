//! Checks that Clippy's results from an incremental re-check match a fresh check.
//!
//! Each directory in `tests/incremental` is a scenario of numbered steps (`1`, `2`, ...), each a
//! complete library crate (`lib.rs`, its modules and optionally `clippy.toml`). A step may also
//! hold `driver-args` (extra `clippy-driver` arguments, one per line) and `driver-env`
//! (`NAME=VALUE` lines); in both, `{src}` stands for the crate's source directory. The steps are
//! checked in order with one incremental cache; after each step, the diagnostics must equal those
//! of a fresh check of the same files. A scenario's steps must not all produce the same
//! diagnostics, otherwise a stale result could not show up as a mismatch.

#![warn(rust_2018_idioms, unused_lifetimes)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod test_utils;

const LEVEL_SOURCE_NOTES: [&str; 3] = ["on by default", "implied by `-D warnings`", "to override `-D warnings`"];

#[test]
fn incremental() {
    if test_utils::IS_RUSTC_TEST_SUITE {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/incremental");
    let failures: Vec<String> = sorted_dirs(&root)
        .iter()
        .filter_map(|scenario| check_scenario(scenario).err())
        .collect();
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

fn check_scenario(scenario: &Path) -> Result<(), String> {
    let name = scenario.file_name().unwrap().to_string_lossy();
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let mut results = Vec::new();
    for step in sorted_dirs(scenario) {
        let step_name = step.file_name().unwrap().to_string_lossy();
        if src.exists() {
            fs::remove_dir_all(&src).unwrap();
        }
        fs::create_dir(&src).unwrap();
        for file in fs::read_dir(&step).unwrap() {
            let file = file.unwrap();
            if file.file_name() != "driver-args" && file.file_name() != "driver-env" {
                fs::copy(file.path(), src.join(file.file_name())).unwrap();
            }
        }
        let options = DriverOptions::read(&step, &src);
        let incremental = check(tmp.path(), &options, true).map_err(|e| format!("{name}, step {step_name}: {e}"))?;
        let fresh = check(tmp.path(), &options, false).map_err(|e| format!("{name}, step {step_name}: {e}"))?;
        if incremental != fresh {
            return Err(format!(
                "{name}, step {step_name}: the incremental check differs from a fresh check\n\
                 --- fresh\n{}\n--- incremental\n{}",
                fresh.join("\n"),
                incremental.join("\n"),
            ));
        }
        results.push(fresh);
    }
    if results.windows(2).all(|pair| pair[0] == pair[1]) {
        return Err(format!(
            "{name}: every step produces the same diagnostics, so a stale result cannot be detected"
        ));
    }
    Ok(())
}

/// A step's extra `clippy-driver` arguments and environment variables.
struct DriverOptions {
    args: Vec<String>,
    env: Vec<(String, String)>,
}

impl DriverOptions {
    fn read(step: &Path, src: &Path) -> Self {
        let src = fs::canonicalize(src).unwrap();
        let lines = |file: &str| -> Vec<String> {
            fs::read_to_string(step.join(file))
                .unwrap_or_default()
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| line.replace("{src}", src.to_str().unwrap()))
                .collect()
        };
        Self {
            args: lines("driver-args"),
            env: lines("driver-env")
                .into_iter()
                .map(|line| {
                    let (name, value) = line.split_once('=').expect("`driver-env` lines are `NAME=VALUE`");
                    (name.to_string(), value.to_string())
                })
                .collect(),
        }
    }
}

/// Checks `dir/src/lib.rs` with `dir/src/clippy.toml` (if any) and returns the rendered
/// diagnostics, sorted. With `incremental`, the cache in `dir/incremental` is used and updated.
/// Notes on where a lint's level comes from are dropped: rustc attaches them to the first
/// diagnostic of each lint, which depends on emission order.
fn check(dir: &Path, options: &DriverOptions, incremental: bool) -> Result<Vec<String>, String> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_clippy-driver"));
    cmd.current_dir(dir)
        .env("CLIPPY_CONF_DIR", dir.join("src"))
        .env_remove("CLIPPY_ARGS")
        .env_remove("CLIPPY_LIBRARY_ROOTS")
        .envs(options.env.iter().map(|(name, value)| (name, value)))
        .args(&options.args)
        .args([
            "src/lib.rs",
            "--crate-type=lib",
            "--crate-name=scenario",
            "--edition=2024",
            "--emit=metadata",
            "--error-format=json",
        ]);
    if incremental {
        cmd.args([
            "-Cincremental=incremental",
            "-Zincremental-verify-ich",
            "--out-dir=out-incremental",
        ]);
    } else {
        cmd.arg("--out-dir=out-fresh");
    }
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    if !output.status.success() {
        return Err(format!("clippy-driver failed\n{stderr}"));
    }
    let mut diagnostics: Vec<String> = stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|diagnostic| {
            let rendered = diagnostic["rendered"].as_str()?;
            Some(
                rendered
                    .lines()
                    .filter(|line| !LEVEL_SOURCE_NOTES.iter().any(|note| line.contains(note)))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        })
        .collect();
    diagnostics.sort();
    Ok(diagnostics)
}

fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}
