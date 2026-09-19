use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not inside a git repository")]
    NotARepository,

    #[error("this command needs a working tree, and {0} is a bare repository")]
    BareRepository(PathBuf),

    #[error("HEAD is detached, so there is no branch to work from")]
    DetachedHead,

    #[error("no branch or commit named {0}")]
    UnknownRevision(String),

    #[error("{0}")]
    Usage(String),

    #[error("git {command} exited with status {code}")]
    GitFailed { command: String, code: i32 },

    #[error("could not run git: {0}")]
    GitMissing(#[source] std::io::Error),

    #[error(transparent)]
    Git(#[from] git2::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn usage(msg: impl Into<String>) -> Self {
        Error::Usage(msg.into())
    }
}

impl From<lexopt::Error> for Error {
    fn from(e: lexopt::Error) -> Self {
        Error::Usage(e.to_string())
    }
}

/// What a command decided to do, so `main` can pick an exit code without
/// commands knowing about exit codes.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The work was done.
    Done,
    /// The user dismissed a picker.
    Cancelled,
    /// There was nothing to act on.
    Empty,
    /// git ran and failed; its own status is worth passing on.
    GitStatus(i32),
}

impl Outcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            Outcome::Done => 0,
            // The conventional code for "interrupted by the user".
            Outcome::Cancelled => 130,
            Outcome::Empty => 1,
            Outcome::GitStatus(code) => *code,
        }
    }
}
