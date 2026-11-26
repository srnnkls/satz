//! The `sq` command: `satz query`, run by the `satz` binary installed beside it.
#![forbid(unsafe_code)]

use std::io;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn satz() -> io::Result<PathBuf> {
    let satz = std::env::current_exe()?
        .canonicalize()?
        .with_file_name("satz");
    if satz.try_exists()? {
        Ok(satz)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no satz binary at {}", satz.display()),
        ))
    }
}

fn main() -> ExitCode {
    let error = match satz() {
        Ok(satz) => Command::new(satz)
            .arg("query")
            .args(std::env::args_os().skip(1))
            .exec(),
        Err(error) => error,
    };
    eprintln!("sq: {error}");
    ExitCode::from(2)
}
