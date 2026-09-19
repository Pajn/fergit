use std::collections::HashSet;

use git2::{Commit, Oid, Sort};

use crate::error::{Error, Result};
use crate::repo::Repo;

#[derive(Debug, Clone)]
pub struct CommitSummary {
    pub id: Oid,
    pub short_id: String,
    pub summary: String,
    pub author: String,
    /// Commit time, seconds since the epoch.
    pub time: i64,
}

/// Comparing every commit on both sides by content costs a diff each. Past
/// this many commits the branches have diverged far enough that the comparison
/// is not worth the wait, and duplicates are unlikely to be what you are
/// looking at anyway.
const EQUIVALENCE_LIMIT: usize = 200;

impl Repo {
    /// Commits on `target` that are not on the current branch, newest first.
    ///
    /// Commits already applied here under a different hash — a previous
    /// cherry-pick, or the same patch landed twice — are left out, so the list
    /// only offers work that would actually change anything.
    pub fn commits_to_pick(&self, target: &str) -> Result<Vec<CommitSummary>> {
        let base = self.head_commit()?;
        let target_commit = self.resolve_commit(target)?;

        let theirs = self.commits_between(base.id(), target_commit.id())?;
        if theirs.len() > EQUIVALENCE_LIMIT {
            return Ok(theirs.iter().map(summarize).collect());
        }

        let ours = self.commits_between(target_commit.id(), base.id())?;
        let already_here = if ours.len() > EQUIVALENCE_LIMIT {
            HashSet::new()
        } else {
            ours.iter().filter_map(|c| self.patch_id(c)).collect()
        };

        Ok(theirs
            .iter()
            .filter(|c| match self.patch_id(c) {
                Some(id) => !already_here.contains(&id),
                None => true,
            })
            .map(summarize)
            .collect())
    }

    /// Commits reachable from `to` but not from `from`, newest first.
    fn commits_between(&self, from: Oid, to: Oid) -> Result<Vec<Commit<'_>>> {
        let mut walk = self.inner.revwalk()?;
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        walk.push(to)?;
        walk.hide(from)?;
        let mut out = Vec::new();
        for oid in walk {
            out.push(self.inner.find_commit(oid?)?);
        }
        Ok(out)
    }

    /// A hash of what a commit changes, ignoring where it sits in history.
    /// Two commits with the same patch id make the same change.
    fn patch_id(&self, commit: &Commit<'_>) -> Option<Oid> {
        let new_tree = commit.tree().ok()?;
        let old_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let diff = self
            .inner
            .diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), None)
            .ok()?;
        diff.patchid(None).ok()
    }

    pub fn head_commit(&self) -> Result<Commit<'_>> {
        self.inner
            .head()
            .and_then(|h| h.peel_to_commit())
            .map_err(|_| Error::DetachedHead)
    }

    pub fn resolve_commit(&self, rev: &str) -> Result<Commit<'_>> {
        self.inner
            .revparse_single(rev)
            .and_then(|obj| obj.peel_to_commit())
            .map_err(|_| Error::UnknownRevision(rev.to_string()))
    }

    /// Recent commits on a branch, for the branch preview.
    pub fn recent_commits(&self, rev: &str, limit: usize) -> Result<Vec<CommitSummary>> {
        let tip = self.resolve_commit(rev)?;
        let mut walk = self.inner.revwalk()?;
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        walk.push(tip.id())?;
        let mut out = Vec::new();
        for oid in walk.take(limit) {
            out.push(summarize(&self.inner.find_commit(oid?)?));
        }
        Ok(out)
    }
}

fn summarize(commit: &Commit<'_>) -> CommitSummary {
    CommitSummary {
        id: commit.id(),
        short_id: commit
            .as_object()
            .short_id()
            .ok()
            .and_then(|buf| buf.as_str().ok().map(str::to_string))
            .unwrap_or_else(|| commit.id().to_string()[..7].to_string()),
        summary: commit.summary().ok().flatten().unwrap_or("").to_string(),
        author: commit.author().name().unwrap_or("").to_string(),
        time: commit.time().seconds(),
    }
}
