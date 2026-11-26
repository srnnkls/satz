//! The `satz` command: `satz query` prints the rows a tq pattern matches in
//! Markdown and text files, and `satz lint` checks them against satz's std
//! rules and the inline rules of `satz.toml`. Results go to stdout and
//! errors to stderr; exit status 0 is clean, 1 is failing findings and 2 a
//! usage, query or config error.
#![forbid(unsafe_code)]

mod config;
mod edits;
mod failure;
mod input;
mod json;
mod lint;
mod query;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use self::failure::Failure;

/// Structural query and lint over text.
#[derive(Debug, Parser)]
#[command(name = "satz", version)]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "The config file [default: satz.toml, if the current directory has one]"
    )]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Match a tq pattern and print one row per match.
    Query(query::Args),
    /// Check files against the lint rules.
    Lint(lint::Args),
}

/// How a command that ran to the end went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// Nothing to report.
    Clean,
    /// Findings that fail the run.
    Findings,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut out = io::stdout().lock();
    let config = config::Source::new(cli.config);
    let result = match cli.command {
        Command::Query(args) => query::run(&args, &config, &mut out),
        Command::Lint(args) => lint::run(&args, &config, &mut out),
    }
    .and_then(|status| out.flush().map(|()| status).map_err(Failure::from));
    match result {
        Ok(Status::Clean) => ExitCode::SUCCESS,
        Ok(Status::Findings) => ExitCode::from(1),
        Err(failure) => {
            eprintln!("satz: {failure}");
            ExitCode::from(2)
        }
    }
}
