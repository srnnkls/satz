//! The `tela` command.
#![forbid(unsafe_code)]

mod config;
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
#[command(name = "tela", version)]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "The config file [default: tela.toml, if the current directory has one]"
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
            eprintln!("tela: {failure}");
            ExitCode::from(2)
        }
    }
}
