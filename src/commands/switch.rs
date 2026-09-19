use crate::error::{Outcome, Result};
use crate::format::{self, relative_time};
use crate::git::Git;
use crate::picker::Prompt;
use crate::repo::Repo;
use crate::repo::branch::{Branch, BranchKind, BranchLookup};
use crate::repo::diff::{LineKind, PatchLine};

#[derive(Debug, Default)]
pub struct Args {
    /// The branch to switch to, if it was named.
    pub branch: Option<String>,
    /// Create the branch even if the name could be resolved.
    pub create: bool,
}

/// Switch branches: to the one named, or to the one picked.
pub fn run(args: Args) -> Result<Outcome> {
    let repo = Repo::discover()?;

    if let Some(name) = args.branch {
        return switch_to_name(&repo, &name, args.create);
    }

    let branches = repo.branches()?;
    if branches.is_empty() {
        println!("This repository has no branches yet.");
        return Ok(Outcome::Empty);
    }

    let picked = Prompt::new(branches)
        .prompt("switch")
        .preview(|branch| log_preview(&repo, branch))
        .run()?
        .into_picked()
        .and_then(|mut picked| picked.pop());

    match picked {
        Some(branch) => switch_to_branch(&repo, &branch),
        None => Ok(Outcome::Cancelled),
    }
}

/// `fergit switch <name>` from the command line.
///
/// The name is resolved the way you would expect to be able to say it: an
/// existing branch is switched to, a name only a remote has is checked out to
/// track that remote, and a name nothing answers to is created.
fn switch_to_name(repo: &Repo, name: &str, force_create: bool) -> Result<Outcome> {
    // git's own shorthand for "wherever I just was".
    if name == "-" {
        return Git::new("switch").arg("-").run();
    }
    if force_create {
        return Git::new("switch").arg("-c").arg(name).run();
    }

    match repo.look_up_branch(name) {
        // git creates the tracking branch itself when exactly one remote has
        // the name, which is the case we checked for.
        BranchLookup::Local | BranchLookup::Trackable => Git::new("switch").arg(name).run(),
        BranchLookup::Unknown => Git::new("switch").arg("-c").arg(name).run(),
    }
}

/// Switch to a branch chosen from the list.
fn switch_to_branch(repo: &Repo, branch: &Branch) -> Result<Outcome> {
    if branch.kind == BranchKind::Local {
        return Git::new("switch").arg(&branch.name).run();
    }

    // A remote-tracking branch is not somewhere you can stand. If a local
    // branch of that name already exists, that is the one meant; otherwise
    // create it, tracking the remote.
    let local = branch.local_name();
    if repo.look_up_branch(local) == BranchLookup::Local {
        return Git::new("switch").arg(local).run();
    }
    Git::new("switch").arg("--track").arg(&branch.name).run()
}

const PREVIEW_COMMITS: usize = 200;

fn log_preview(repo: &Repo, branch: &Branch) -> Vec<PatchLine> {
    let now = format::now();
    match repo.recent_commits(&branch.name, PREVIEW_COMMITS) {
        Ok(commits) => commits
            .into_iter()
            .map(|commit| PatchLine {
                kind: LineKind::Context,
                text: format!(
                    "{}  {}  ({}, {})",
                    commit.short_id,
                    commit.summary,
                    commit.author,
                    relative_time(commit.time, now)
                ),
            })
            .collect(),
        Err(err) => vec![PatchLine {
            kind: LineKind::Meta,
            text: format!("Could not read this branch: {err}"),
        }],
    }
}
