use crate::error::{Error, Outcome, Result};
use crate::git::Git;
use crate::picker::{Prompt, Selection};
use crate::repo::Repo;
use crate::repo::branch::Branch;
use crate::repo::commit::CommitSummary;
use crate::repo::diff::{LineKind, PatchLine};

#[derive(Debug, Default)]
pub struct Args {
    /// Branches or commits, depending on what they turn out to name.
    pub refs: Vec<String>,
    /// Pick commits from this branch, said explicitly.
    pub from: Option<String>,
    /// Treat every argument as a commit, even if a branch has that name.
    pub direct: bool,
    /// Arguments after `--`, forwarded to git cherry-pick.
    pub git_args: Vec<String>,
}

/// Cherry-pick: the commits named, or the ones picked from another branch.
pub fn run(args: Args) -> Result<Outcome> {
    let repo = Repo::discover()?;
    // Cherry-picking onto a detached HEAD works, but "which commits am I
    // missing" has no answer without a branch to compare against.
    if repo.head_branch().is_none() {
        return Err(Error::DetachedHead);
    }

    if let Some(branch) = &args.from {
        return pick_commits(&repo, branch, &args.git_args);
    }

    match args.refs.as_slice() {
        [] => browse(&repo, &args.git_args),
        // One branch name means "show me what is on it"; anything else is a
        // commit that was already chosen.
        [name] if !args.direct && is_branch(&repo, name) => {
            pick_commits(&repo, name, &args.git_args)
        }
        refs => cherry_pick(refs, &args.git_args),
    }
}

fn is_branch(repo: &Repo, name: &str) -> bool {
    use crate::repo::branch::BranchLookup;
    match repo.look_up_branch(name) {
        BranchLookup::Local | BranchLookup::Trackable => true,
        // `origin/main` names a branch too, just not by its short name.
        BranchLookup::Unknown => repo
            .branches()
            .map(|branches| branches.iter().any(|b| b.name == name))
            .unwrap_or(false),
    }
}

/// Choose a branch, then commits from it. Backing out of the commits returns
/// to the branch list rather than abandoning the whole thing.
fn browse(repo: &Repo, git_args: &[String]) -> Result<Outcome> {
    let current = repo.head_branch();
    loop {
        let branches: Vec<Branch> = repo
            .branches()?
            .into_iter()
            // Picking commits from the branch you are on would offer nothing.
            .filter(|b| Some(&b.name) != current.as_ref())
            .collect();
        if branches.is_empty() {
            println!("There are no other branches to pick from.");
            return Ok(Outcome::Empty);
        }

        let picked = Prompt::new(branches)
            .prompt("pick from")
            .preview(|branch| commits_preview(repo, &branch.name))
            .run()?
            .into_picked()
            .and_then(|mut picked| picked.pop());

        let Some(branch) = picked else {
            return Ok(Outcome::Cancelled);
        };
        match pick_commits(repo, &branch.name, git_args)? {
            // Backing out of the commit list means "wrong branch", not "never
            // mind".
            Outcome::Cancelled => continue,
            outcome => return Ok(outcome),
        }
    }
}

fn pick_commits(repo: &Repo, branch: &str, git_args: &[String]) -> Result<Outcome> {
    let commits = repo.commits_to_pick(branch)?;
    if commits.is_empty() {
        println!("Nothing on {branch} that is not already here.");
        return Ok(Outcome::Empty);
    }

    let selection = Prompt::new(commits)
        .prompt("cherry-pick")
        .multi()
        .preview(|commit: &CommitSummary| patch_preview(repo, commit))
        .run()?;

    let Selection::Picked(picked) = selection else {
        return Ok(Outcome::Cancelled);
    };

    // The list runs newest first; commits have to be applied the other way
    // round, whatever order they were marked in.
    let commits: Vec<String> = picked
        .iter()
        .rev()
        .map(|commit| commit.id.to_string())
        .collect();
    cherry_pick(&commits, git_args)
}

fn cherry_pick(refs: &[String], git_args: &[String]) -> Result<Outcome> {
    Git::new("cherry-pick").args(git_args).args(refs).run()
}

fn patch_preview(repo: &Repo, commit: &CommitSummary) -> Vec<PatchLine> {
    repo.commit_patch(commit.id).unwrap_or_else(|err| {
        vec![PatchLine {
            kind: LineKind::Meta,
            text: format!("Could not read this commit: {err}"),
        }]
    })
}

/// What picking this branch would offer: the commits it has that we do not.
fn commits_preview(repo: &Repo, branch: &str) -> Vec<PatchLine> {
    match repo.commits_to_pick(branch) {
        Ok(commits) if commits.is_empty() => vec![PatchLine {
            kind: LineKind::Meta,
            text: "Nothing here that is not already on this branch.".into(),
        }],
        Ok(commits) => commits
            .into_iter()
            .map(|commit| PatchLine {
                kind: LineKind::Context,
                text: format!("{}  {}", commit.short_id, commit.summary),
            })
            .collect(),
        Err(err) => vec![PatchLine {
            kind: LineKind::Meta,
            text: format!("Could not compare with this branch: {err}"),
        }],
    }
}
