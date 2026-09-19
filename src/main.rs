mod cli;
mod commands;
mod error;
mod format;
mod git;
mod picker;
mod repo;
mod rows;

use std::process::ExitCode;

use cli::Command;
use error::{Outcome, Result};

fn main() -> ExitCode {
    match run() {
        Ok(outcome) => ExitCode::from(outcome.exit_code() as u8),
        Err(error) => {
            eprintln!("\x1b[31mfergit:\x1b[0m {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<Outcome> {
    match cli::parse(std::env::args_os().collect())? {
        Command::Add(args) => commands::add::run(args),
        Command::Switch(args) => commands::switch::run(args),
        Command::CherryPick(args) => commands::cherry_pick::run(args),
        Command::Install(args) => commands::install::run(args),
        Command::Aliases => commands::install::aliases(),
        Command::Help => {
            print!("{}", cli::HELP);
            Ok(Outcome::Done)
        }
        Command::Version => {
            println!("fergit {}", env!("CARGO_PKG_VERSION"));
            Ok(Outcome::Done)
        }
    }
}
