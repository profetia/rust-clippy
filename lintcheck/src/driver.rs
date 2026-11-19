use crate::recursive::{DriverInfo, deserialize_line, serialize_line};

use std::io::{self, BufReader, Write as _};
use std::net::TcpStream;
use std::path::Path;
use std::process::{self, Command, Stdio};
use std::{env, mem};

/// 1. Sends [`DriverInfo`] to the [`crate::recursive::LintcheckServer`] running on `addr`
/// 2. Receives [bool] from the server, if `false` returns `None`
/// 3. Otherwise sends the stderr of running `clippy-driver` to the server
fn run_clippy(addr: &str) -> Option<i32> {
    let driver_info = DriverInfo {
        package_name: env::var("CARGO_PKG_NAME").ok()?,
        version: env::var("CARGO_PKG_VERSION").ok()?,
    };

    let mut stream = BufReader::new(TcpStream::connect(addr).unwrap());

    serialize_line(&driver_info, stream.get_mut());

    let should_run = deserialize_line::<bool, _>(&mut stream);
    if !should_run {
        return None;
    }

    // Remove --cap-lints allow so that clippy runs and lints are emitted
    let mut include_next = true;
    let args = env::args().skip(1).filter(|arg| match arg.as_str() {
        "--cap-lints=allow" => false,
        "--cap-lints" => {
            include_next = false;
            false
        },
        _ => mem::replace(&mut include_next, true),
    });

    let output = Command::new(env::var("CLIPPY_DRIVER").expect("missing env CLIPPY_DRIVER"))
        .args(args)
        .stdout(Stdio::inherit())
        .output()
        .expect("failed to run clippy-driver");

    stream
        .get_mut()
        .write_all(&output.stderr)
        .unwrap_or_else(|e| panic!("{e:?} in {driver_info:?}"));

    match output.status.code() {
        Some(0) => Some(0),
        code => {
            io::stderr().write_all(&output.stderr).unwrap();
            Some(code.expect("killed by signal"))
        },
    }
}

pub fn drive(addr: &str) {
    process::exit(run_clippy(addr).unwrap_or_else(|| {
        Command::new("rustc")
            .args(env::args_os().skip(2))
            .status()
            .unwrap()
            .code()
            .unwrap()
    }))
}

fn arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

/// Runs `clippy-driver`, recording it with `perf` into
/// `perf.data.<crate-name>-<lib|bin>.<run_number>` in the package directory if it compiles a
/// lib or bin target of the primary package
pub fn drive_perf(run_number: &str) {
    let clippy_driver = env::var("CLIPPY_DRIVER").expect("missing env CLIPPY_DRIVER");
    let args: Vec<String> = env::args().skip(1).collect();

    let mut cmd = match arg_value(&args, "--crate-name") {
        Some(crate_name) if env::var_os("CARGO_PRIMARY_PACKAGE").is_some() && crate_name != "build_script_build" => {
            let kind = if arg_value(&args, "--crate-type") == Some("bin") {
                "bin"
            } else {
                "lib"
            };
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing env CARGO_MANIFEST_DIR");
            let perf_data_path = Path::new(&manifest_dir).join(format!("perf.data.{crate_name}-{kind}.{run_number}"));

            let mut cmd = Command::new("perf");
            cmd.args(crate::PERF_RECORD_ARGS)
                .arg("-o")
                .arg(perf_data_path)
                .arg("--")
                .arg(&clippy_driver);
            cmd
        },
        _ => Command::new(&clippy_driver),
    };

    let status = cmd.args(&args).status().expect("failed to run clippy-driver");
    process::exit(status.code().expect("killed by signal"));
}
