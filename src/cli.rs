use std::ffi::OsString;
use std::path::{Path, PathBuf};

use lexopt::prelude::*;

use crate::commands::{add, cherry_pick, install, switch};
use crate::error::{Error, Result};

#[derive(Debug)]
pub enum Command {
    Add(add::Args),
    Switch(switch::Args),
    CherryPick(cherry_pick::Args),
    Install(install::Args),
    Aliases,
    Help,
    Version,
}

pub const HELP: &str = "\
fergit — interactive git commands in one binary

USAGE
    fergit <command> [args]     or through a symlink: ga, gcb, gcp

COMMANDS
    add, ga                 Stage changes from the working tree
      fergit add                  pick files to stage
      fergit add <path>...        stage paths, no picker
      any other flag is passed to git add, so -p still works

    switch, gcb             Switch branches
      fergit switch               pick a branch
      fergit switch <branch>      switch to it, creating it if it is new
      fergit switch -             switch back to the previous branch
      -c, --create                create the branch even if the name resolves

    cherry-pick, gcp        Take commits from another branch
      fergit cherry-pick          pick a branch, then commits from it
      fergit cherry-pick <branch> pick commits from that branch
      fergit cherry-pick <rev>... cherry-pick those commits, no picker
      -f, --from <branch>         pick commits from <branch>
      -n, --no-select             treat every argument as a commit
      -- <args>                   pass <args> to git cherry-pick

    install [dir] [--force] Symlink ga, gcb and gcp next to this binary
    aliases                 Print shell aliases instead

IN A PICKER
    type                    filter
    up/down, ctrl-p/n       move
    tab                     mark (where several can be picked)
    ctrl-a                  mark or unmark everything matching
    alt-j/alt-k             scroll the preview
    ctrl-o                  hide or show the preview
    enter                   accept        esc, ctrl-c   cancel
";

/// Work out what to run from the command line, including the case where the
/// binary was invoked through one of its symlinks.
pub fn parse(argv: Vec<OsString>) -> Result<Command> {
    let mut argv = argv.into_iter();
    let program = argv.next().unwrap_or_default();
    let rest: Vec<OsString> = argv.collect();

    if let Some(command) = alias_for(&program) {
        return parse_command(command, rest);
    }

    let mut rest = rest.into_iter();
    let Some(name) = rest.next() else {
        return Ok(Command::Help);
    };
    parse_command(&name.to_string_lossy(), rest.collect())
}

/// The command a program name stands for, when fergit is invoked as `ga`,
/// `gcb` or `gcp`.
fn alias_for(program: &OsString) -> Option<&'static str> {
    let name = Path::new(program)
        .file_name()?
        .to_string_lossy()
        .into_owned();
    install::LINKS
        .iter()
        .find(|(alias, _)| *alias == name)
        .map(|(_, command)| *command)
}

fn parse_command(name: &str, args: Vec<OsString>) -> Result<Command> {
    match name {
        "add" | "ga" => Ok(Command::Add(parse_add(args))),
        "switch" | "checkout-branch" | "gcb" => Ok(Command::Switch(parse_switch(args)?)),
        "cherry-pick" | "gcp" => Ok(Command::CherryPick(parse_cherry_pick(args)?)),
        "install" => Ok(Command::Install(parse_install(args)?)),
        "aliases" => Ok(Command::Aliases),
        "help" | "-h" | "--help" => Ok(Command::Help),
        "version" | "-V" | "--version" => Ok(Command::Version),
        other => Err(Error::usage(format!(
            "unknown command: {other} (try --help)"
        ))),
    }
}

/// `add` takes no options of its own: anything that looks like a flag belongs
/// to git, and anything else is a path. Splitting rather than parsing keeps
/// every git flag working without fergit having to know about it.
fn parse_add(args: Vec<OsString>) -> add::Args {
    let mut parsed = add::Args::default();
    let mut paths_only = false;
    for arg in args {
        let text = arg.to_string_lossy().into_owned();
        if paths_only {
            parsed.paths.push(text);
        } else if text == "--" {
            paths_only = true;
        } else if text.starts_with('-') && text != "-" {
            parsed.flags.push(text);
        } else {
            parsed.paths.push(text);
        }
    }
    parsed
}

fn parse_switch(args: Vec<OsString>) -> Result<switch::Args> {
    let mut parsed = switch::Args::default();
    let mut parser = lexopt::Parser::from_args(args);
    while let Some(arg) = parser.next()? {
        match arg {
            Short('c') | Long("create") => parsed.create = true,
            Value(value) if parsed.branch.is_none() => {
                parsed.branch = Some(value.string()?);
            }
            Value(value) => {
                return Err(Error::usage(format!(
                    "switch takes one branch, and {:?} is a second",
                    value.to_string_lossy()
                )));
            }
            other => return Err(Error::usage(other.unexpected().to_string())),
        }
    }
    Ok(parsed)
}

fn parse_cherry_pick(args: Vec<OsString>) -> Result<cherry_pick::Args> {
    let mut parsed = cherry_pick::Args::default();

    // Split on `--` first: what follows belongs to git, and a parser would
    // fold it into our own positionals.
    let mut args = args;
    if let Some(at) = args.iter().position(|a| a == "--") {
        parsed.git_args = args
            .split_off(at + 1)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        args.pop();
    }

    let mut parser = lexopt::Parser::from_args(args);
    while let Some(arg) = parser.next()? {
        match arg {
            Short('f') | Long("from") => parsed.from = Some(parser.value()?.string()?),
            Short('n') | Long("no-select") => parsed.direct = true,
            Value(value) => parsed.refs.push(value.string()?),
            other => return Err(Error::usage(other.unexpected().to_string())),
        }
    }
    Ok(parsed)
}

fn parse_install(args: Vec<OsString>) -> Result<install::Args> {
    let mut parsed = install::Args::default();
    let mut parser = lexopt::Parser::from_args(args);
    while let Some(arg) = parser.next()? {
        match arg {
            Short('f') | Long("force") => parsed.force = true,
            Value(value) => parsed.dir = Some(PathBuf::from(value)),
            other => return Err(Error::usage(other.unexpected().to_string())),
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_symlink_name_selects_the_command() {
        let parsed = parse(argv(&["/usr/local/bin/gcb", "main"])).unwrap();
        assert!(matches!(
            parsed,
            Command::Switch(switch::Args { branch: Some(b), .. }) if b == "main"
        ));
    }

    #[test]
    fn the_binary_name_falls_through_to_a_subcommand() {
        let parsed = parse(argv(&["fergit", "add", "src/"])).unwrap();
        assert!(matches!(parsed, Command::Add(_)));
    }

    #[test]
    fn no_arguments_asks_for_help() {
        assert!(matches!(parse(argv(&["fergit"])).unwrap(), Command::Help));
    }

    #[test]
    fn add_separates_git_flags_from_paths() {
        let parsed = parse_add(argv(&["-p", "src/main.rs", "--verbose", "Cargo.toml"]));
        assert_eq!(parsed.flags, vec!["-p", "--verbose"]);
        assert_eq!(parsed.paths, vec!["src/main.rs", "Cargo.toml"]);
    }

    #[test]
    fn add_treats_everything_after_a_double_dash_as_a_path() {
        let parsed = parse_add(argv(&["-p", "--", "-weird-name.rs"]));
        assert_eq!(parsed.flags, vec!["-p"]);
        assert_eq!(parsed.paths, vec!["-weird-name.rs"]);
    }

    #[test]
    fn switch_accepts_a_dash_as_a_branch() {
        let parsed = parse_switch(argv(&["-"])).unwrap();
        assert_eq!(parsed.branch.as_deref(), Some("-"));
    }

    #[test]
    fn switch_refuses_a_second_branch() {
        assert!(parse_switch(argv(&["a", "b"])).is_err());
    }

    #[test]
    fn cherry_pick_splits_our_flags_from_gits() {
        let parsed = parse_cherry_pick(argv(&["-n", "abc123", "--", "-x", "--no-commit"])).unwrap();
        assert!(parsed.direct);
        assert_eq!(parsed.refs, vec!["abc123"]);
        assert_eq!(parsed.git_args, vec!["-x", "--no-commit"]);
    }

    #[test]
    fn cherry_pick_reads_the_from_branch() {
        let parsed = parse_cherry_pick(argv(&["--from", "origin/main"])).unwrap();
        assert_eq!(parsed.from.as_deref(), Some("origin/main"));
        assert!(parsed.refs.is_empty());
    }

    #[test]
    fn an_unknown_command_is_an_error() {
        assert!(parse(argv(&["fergit", "frobnicate"])).is_err());
    }
}
