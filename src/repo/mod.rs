pub mod branch;
pub mod commit;
pub mod diff;
pub mod status;

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The repository the command is running against.
///
/// Everything fergit reads — status, branches, history, patches — comes from
/// libgit2 through this type. Operations that change the repository are not
/// here: they run `git` itself, so that hooks, the sequencer, and `--continue`
/// behave exactly as they would if you had typed the command.
pub struct Repo {
    pub(crate) inner: git2::Repository,
    workdir: PathBuf,
}

impl Repo {
    /// Find the repository containing the current directory.
    pub fn discover() -> Result<Self> {
        let inner = git2::Repository::discover(".").map_err(|_| Error::NotARepository)?;
        let workdir = inner
            .workdir()
            .ok_or_else(|| Error::BareRepository(inner.path().to_path_buf()))?
            .to_path_buf();
        Ok(Repo { inner, workdir })
    }

    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    pub(crate) fn config_string(&self, key: &str) -> Option<String> {
        self.inner.config().ok()?.get_string(key).ok()
    }

    /// A repository path as it should be shown to someone standing in the
    /// current directory.
    pub fn display_path(&self, repo_relative: &Path) -> String {
        let absolute = self.workdir.join(repo_relative);
        match std::env::current_dir() {
            Ok(cwd) => crate::format::relative_path(&cwd, &absolute),
            Err(_) => absolute.to_string_lossy().into_owned(),
        }
    }
}
