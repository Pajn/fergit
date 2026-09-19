use crate::error::{Outcome, Result};
use crate::git::{self, Git};
use crate::picker::Prompt;
use crate::repo::Repo;
use crate::repo::diff::{LineKind, PatchLine};
use crate::rows::FileRow;

#[derive(Debug, Default)]
pub struct Args {
    /// Paths to stage without opening a picker.
    pub paths: Vec<String>,
    /// Flags passed straight to git, such as `-p`.
    pub flags: Vec<String>,
}

/// Stage changes: the ones named, or the ones picked.
pub fn run(args: Args) -> Result<Outcome> {
    let repo = Repo::discover()?;

    if !args.paths.is_empty() {
        return stage(&args.flags, &args.paths);
    }

    let files: Vec<FileRow> = repo
        .changed_files()?
        .into_iter()
        .map(|file| FileRow {
            display: repo.display_path(&file.path),
            file,
        })
        .collect();

    if files.is_empty() {
        println!("Nothing to add.");
        return Ok(Outcome::Empty);
    }

    let picked = Prompt::new(files)
        .prompt("add")
        .multi()
        .preview(|row| patch_or_reason(&repo, row))
        .run()?
        .into_picked();

    let Some(picked) = picked else {
        return Ok(Outcome::Cancelled);
    };

    // Absolute paths, so the command means the same thing from any directory
    // inside the repository.
    let paths: Vec<_> = picked
        .iter()
        .map(|row| repo.workdir().join(&row.file.path))
        .collect();

    let outcome = Git::new("add").args(&args.flags).paths(&paths).run()?;
    if outcome == Outcome::Done {
        git::print_short_status()?;
    }
    Ok(outcome)
}

fn stage(flags: &[String], paths: &[String]) -> Result<Outcome> {
    let outcome = Git::new("add").args(flags).paths(paths).run()?;
    if outcome == Outcome::Done {
        git::print_short_status()?;
    }
    Ok(outcome)
}

/// A preview should never be the reason a picker fails, so a patch that cannot
/// be produced is reported in the pane instead.
fn patch_or_reason(repo: &Repo, row: &FileRow) -> Vec<PatchLine> {
    repo.file_patch(&row.file).unwrap_or_else(|err| {
        vec![PatchLine {
            kind: LineKind::Meta,
            text: format!("Could not diff this file: {err}"),
        }]
    })
}
