use git2::BranchType;

use crate::error::Result;
use crate::repo::Repo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
    Local,
    /// A remote-tracking branch, e.g. `origin/main`.
    Remote,
}

#[derive(Debug, Clone)]
pub struct Branch {
    /// Short name: `main`, or `origin/main` for a remote-tracking branch.
    pub name: String,
    pub kind: BranchKind,
    pub is_head: bool,
    /// The branch this one tracks, for local branches that track one.
    pub upstream: Option<String>,
    /// Commit time of the tip, used to put recent work first.
    pub tip_time: i64,
    pub subject: String,
}

/// What a name given on the command line turns out to refer to.
#[derive(Debug, PartialEq, Eq)]
pub enum BranchLookup {
    /// A branch that already exists locally.
    Local,
    /// Not local, but exactly one remote has it, so git can create a tracking
    /// branch without being told which remote to use.
    Trackable,
    /// Not a branch anywhere.
    Unknown,
}

impl Repo {
    /// Every branch, the current one first, then the rest by how recently they
    /// were committed to.
    pub fn branches(&self) -> Result<Vec<Branch>> {
        let mut branches: Vec<Branch> = self
            .inner
            .branches(None)?
            .filter_map(|b| b.ok())
            .filter_map(|(branch, kind)| self.describe_branch(&branch, kind))
            .collect();

        branches.sort_by(|a, b| {
            b.is_head
                .cmp(&a.is_head)
                .then_with(|| a.kind.cmp_order().cmp(&b.kind.cmp_order()))
                .then_with(|| b.tip_time.cmp(&a.tip_time))
        });
        Ok(branches)
    }

    fn describe_branch(&self, branch: &git2::Branch<'_>, kind: BranchType) -> Option<Branch> {
        let name = branch.name().ok().flatten()?.to_string();
        // `origin/HEAD` is a pointer at another branch in the list, not a
        // branch anyone means to check out.
        if kind == BranchType::Remote && name.ends_with("/HEAD") {
            return None;
        }
        let commit = branch.get().peel_to_commit().ok()?;
        Some(Branch {
            name,
            kind: match kind {
                BranchType::Local => BranchKind::Local,
                BranchType::Remote => BranchKind::Remote,
            },
            is_head: branch.is_head(),
            upstream: branch
                .upstream()
                .ok()
                .and_then(|u| u.name().ok().flatten().map(str::to_string)),
            tip_time: commit.time().seconds(),
            subject: commit.summary().ok().flatten().unwrap_or("").to_string(),
        })
    }

    /// Decide what `fergit switch <name>` should do with a name.
    pub fn look_up_branch(&self, name: &str) -> BranchLookup {
        if self.inner.find_branch(name, BranchType::Local).is_ok() {
            return BranchLookup::Local;
        }
        let remotes = self.inner.remotes().map(|r| {
            r.iter()
                .filter_map(|name| name.ok().flatten())
                .filter(|remote| {
                    self.inner
                        .find_branch(&format!("{remote}/{name}"), BranchType::Remote)
                        .is_ok()
                })
                .count()
        });
        // More than one remote with the name is ambiguous, and git will say so
        // more clearly than we can.
        match remotes {
            Ok(1) => BranchLookup::Trackable,
            _ => BranchLookup::Unknown,
        }
    }

    pub fn head_branch(&self) -> Option<String> {
        let head = self.inner.head().ok()?;
        head.is_branch()
            .then(|| head.shorthand().ok().map(str::to_string))
            .flatten()
    }
}

impl BranchKind {
    /// Local branches sort ahead of remote ones.
    fn cmp_order(self) -> u8 {
        match self {
            BranchKind::Local => 0,
            BranchKind::Remote => 1,
        }
    }
}

impl Branch {
    /// The local branch name a remote-tracking branch would be checked out as.
    pub fn local_name(&self) -> &str {
        match self.kind {
            BranchKind::Local => &self.name,
            BranchKind::Remote => self.name.split_once('/').map_or(&*self.name, |(_, b)| b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(name: &str, kind: BranchKind) -> Branch {
        Branch {
            name: name.into(),
            kind,
            is_head: false,
            upstream: None,
            tip_time: 0,
            subject: String::new(),
        }
    }

    #[test]
    fn strips_the_remote_from_a_tracking_branch_name() {
        assert_eq!(
            branch("origin/main", BranchKind::Remote).local_name(),
            "main"
        );
        assert_eq!(
            branch("origin/feat/nested", BranchKind::Remote).local_name(),
            "feat/nested"
        );
        assert_eq!(branch("main", BranchKind::Local).local_name(), "main");
    }
}
