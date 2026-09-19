use std::path::PathBuf;

use crate::error::{Error, Outcome, Result};

/// The names fergit answers to when invoked through a symlink, and the command
/// each one runs.
pub const LINKS: [(&str, &str); 3] = [("ga", "add"), ("gcb", "switch"), ("gcp", "cherry-pick")];

#[derive(Debug, Default)]
pub struct Args {
    pub dir: Option<PathBuf>,
    pub force: bool,
}

/// Create the short command names as symlinks to this binary.
pub fn run(args: Args) -> Result<Outcome> {
    let exe = std::env::current_exe()?;
    let dir = match args.dir {
        Some(dir) => dir,
        None => exe.parent().map(PathBuf::from).unwrap_or_default(),
    };
    if !dir.is_dir() {
        return Err(Error::usage(format!("not a directory: {}", dir.display())));
    }

    for (name, _) in LINKS {
        let link = dir.join(name);
        if link.exists() || link.is_symlink() {
            if !args.force {
                println!("skipped {} (already exists)", link.display());
                continue;
            }
            std::fs::remove_file(&link)?;
        }
        std::os::unix::fs::symlink(&exe, &link)?;
        println!("linked {} -> {}", link.display(), exe.display());
    }
    Ok(Outcome::Done)
}

/// Print shell aliases, for anyone who would rather not have symlinks.
pub fn aliases() -> Result<Outcome> {
    let exe = std::env::current_exe()?;
    for (name, command) in LINKS {
        println!("alias {name}='{} {command}'", exe.display());
    }
    Ok(Outcome::Done)
}
