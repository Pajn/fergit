use std::ffi::{OsStr, OsString};
use std::process::Command;

use crate::error::{Error, Outcome, Result};

/// A git command to run.
///
/// Reads go through libgit2; this is the other half. Anything that changes the
/// repository is handed to git itself so that hooks fire, the sequencer is
/// written, and `git cherry-pick --continue` picks up where a conflict left
/// off — none of which libgit2 does on its own.
pub struct Git {
    args: Vec<OsString>,
}

impl Git {
    pub fn new(subcommand: &str) -> Self {
        Git {
            args: vec![OsString::from(subcommand)],
        }
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
        self
    }

    /// Separate flags from paths, so a file named `-f` cannot become one.
    pub fn paths<I, S>(self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.arg("--").args(paths)
    }

    /// Run git with the terminal attached, so its output and any editor it
    /// opens behave normally.
    pub fn run(self) -> Result<Outcome> {
        let name = self.description();
        let status = Command::new("git")
            .args(&self.args)
            .status()
            .map_err(Error::GitMissing)?;

        match status.code() {
            Some(0) => Ok(Outcome::Done),
            Some(code) => Ok(Outcome::GitStatus(code)),
            // Killed by a signal: report the shell's convention for it.
            None => Err(Error::GitFailed {
                command: name,
                code: 128,
            }),
        }
    }

    fn description(&self) -> String {
        self.args
            .first()
            .map(|a| a.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Print the short status, the way git does after a command that changed it.
pub fn print_short_status() -> Result<Outcome> {
    Git::new("status").arg("--short").run()
}
