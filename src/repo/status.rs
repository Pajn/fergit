use std::path::{Path, PathBuf};

use git2::{Status, StatusEntry, StatusOptions};

use crate::error::Result;
use crate::repo::Repo;

/// What happened to a file on one side of the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Unchanged,
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChange,
    Untracked,
    Conflicted,
}

impl FileState {
    /// The letter `git status --short` uses for this state.
    pub fn code(self) -> char {
        match self {
            FileState::Unchanged => ' ',
            FileState::Added => 'A',
            FileState::Modified => 'M',
            FileState::Deleted => 'D',
            FileState::Renamed => 'R',
            FileState::TypeChange => 'T',
            FileState::Untracked => '?',
            FileState::Conflicted => 'U',
        }
    }
}

/// A file with changes in the working tree. Files whose only changes are
/// already staged are not represented here: there is nothing left to add.
#[derive(Debug, Clone)]
pub struct ChangedFile {
    /// Path relative to the repository root, as git records it.
    pub path: PathBuf,
    /// Where the content came from, for a rename or copy.
    pub origin: Option<PathBuf>,
    pub index: FileState,
    pub worktree: FileState,
}

impl ChangedFile {
    /// The two-letter status code, index side first.
    pub fn code(&self) -> String {
        format!("{}{}", self.index.code(), self.worktree.code())
    }
}

impl Repo {
    /// Files changed in the working tree, in the order git reports them.
    ///
    /// Untracked files are listed individually rather than collapsed into
    /// their directory, unless `status.showUntrackedFiles` says otherwise:
    /// staging a directory is rarely what you meant to pick.
    pub fn changed_files(&self) -> Result<Vec<ChangedFile>> {
        let untracked = self.untracked_files_setting();
        let mut opts = StatusOptions::new();
        opts.include_untracked(untracked != UntrackedFiles::No)
            .recurse_untracked_dirs(untracked == UntrackedFiles::All)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true)
            .include_ignored(false)
            .include_unmodified(false)
            .exclude_submodules(false);

        let statuses = self.inner.statuses(Some(&mut opts))?;
        Ok(statuses
            .iter()
            .filter_map(|entry| ChangedFile::from_entry(&entry))
            .collect())
    }

    fn untracked_files_setting(&self) -> UntrackedFiles {
        match self.config_string("status.showUntrackedFiles").as_deref() {
            Some("no") => UntrackedFiles::No,
            Some("normal") => UntrackedFiles::Normal,
            // Listing every untracked file is the useful default for a picker,
            // and matches what git itself does unless told otherwise.
            _ => UntrackedFiles::All,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum UntrackedFiles {
    No,
    Normal,
    All,
}

impl ChangedFile {
    fn from_entry(entry: &StatusEntry<'_>) -> Option<ChangedFile> {
        let status = entry.status();
        let index = index_state(status);
        let worktree = worktree_state(status);
        // Nothing to stage when the working tree matches the index.
        if worktree == FileState::Unchanged {
            return None;
        }

        // Prefer the working-tree delta: it carries the path as it exists now.
        let delta = entry.index_to_workdir().or_else(|| entry.head_to_index())?;
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())?
            .to_path_buf();
        let origin = matches!(index, FileState::Renamed)
            .then(|| delta.old_file().path().map(Path::to_path_buf))
            .flatten()
            .filter(|old| *old != path);

        Some(ChangedFile {
            path,
            origin,
            index,
            worktree,
        })
    }
}

fn index_state(status: Status) -> FileState {
    if status.is_conflicted() {
        FileState::Conflicted
    } else if status.is_index_new() {
        FileState::Added
    } else if status.is_index_modified() {
        FileState::Modified
    } else if status.is_index_deleted() {
        FileState::Deleted
    } else if status.is_index_renamed() {
        FileState::Renamed
    } else if status.is_index_typechange() {
        FileState::TypeChange
    } else {
        FileState::Unchanged
    }
}

fn worktree_state(status: Status) -> FileState {
    if status.is_conflicted() {
        FileState::Conflicted
    } else if status.is_wt_new() {
        FileState::Untracked
    } else if status.is_wt_modified() {
        FileState::Modified
    } else if status.is_wt_deleted() {
        FileState::Deleted
    } else if status.is_wt_renamed() {
        FileState::Renamed
    } else if status.is_wt_typechange() {
        FileState::TypeChange
    } else {
        FileState::Unchanged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_sides_of_the_index() {
        let both = Status::INDEX_MODIFIED | Status::WT_MODIFIED;
        assert_eq!(index_state(both), FileState::Modified);
        assert_eq!(worktree_state(both), FileState::Modified);
    }

    #[test]
    fn an_untracked_file_has_no_index_side() {
        assert_eq!(index_state(Status::WT_NEW), FileState::Unchanged);
        assert_eq!(worktree_state(Status::WT_NEW), FileState::Untracked);
    }

    #[test]
    fn a_conflict_shows_on_both_sides() {
        assert_eq!(index_state(Status::CONFLICTED), FileState::Conflicted);
        assert_eq!(worktree_state(Status::CONFLICTED), FileState::Conflicted);
    }

    #[test]
    fn formats_the_short_status_code() {
        let file = ChangedFile {
            path: PathBuf::from("a"),
            origin: None,
            index: FileState::Added,
            worktree: FileState::Modified,
        };
        assert_eq!(file.code(), "AM");
    }
}
