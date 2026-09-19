# fergit

Interactive git commands as a single binary.

The utilities are the ones [forgit](https://github.com/wfxr/forgit) made a habit
— pick files to stage, pick a branch to switch to, pick commits to cherry-pick
— rebuilt as a Rust program rather than shell functions around fzf. Nothing is
sourced at shell startup, nothing is piped through a subprocess to draw a list,
and the commands are free to differ from forgit's where a different behaviour is
better.

| Command | Short name | What it does |
| --- | --- | --- |
| `add` | `ga` | Stage changes from the working tree |
| `switch` | `gcb` | Switch branches |
| `cherry-pick` | `gcp` | Take commits from another branch |

## Requirements

- `git` 2.23 or newer, for `git switch`
- Rust 1.85 or newer to build, for the 2024 edition
- A C toolchain, which `git2` needs to build the bundled libgit2

## Install

```sh
cargo build --release
```

Put the binary on your `PATH` and create the short names, either as symlinks:

```sh
fergit install ~/.local/bin
```

or as shell aliases, if you would rather not have symlinks:

```sh
fergit aliases >> ~/.zshrc
```

The binary dispatches on the name it was invoked as, so a symlink needs no
arguments of its own.

## Usage

Every command works both ways: with no arguments it opens a picker, and with
arguments it does the obvious thing without one.

```sh
ga                       # pick files to stage
ga src/ Cargo.toml       # stage these paths
ga -p                    # pick files, then stage them in patch mode

gcb                      # pick a branch to switch to
gcb my-feature           # switch to my-feature, creating it if it is new
gcb -                    # switch back to the previous branch
gcb -c my-feature        # create it, even if the name already resolves

gcp                      # pick a branch, then commits from it
gcp origin/main          # pick commits from origin/main
gcp a1b2c3d 9f8e7d6      # cherry-pick these commits
gcp -n main              # treat main as a commit, not a branch to pick from
gcp abc123 -- -x         # pass -x to git cherry-pick
```

In a picker:

| Key | |
| --- | --- |
| type | filter |
| up/down, ctrl-p/ctrl-n | move |
| tab | mark, where several can be picked |
| ctrl-a | mark or unmark everything matching |
| alt-j/alt-k | scroll the preview |
| ctrl-o | hide or show the preview |
| enter | accept |
| esc, ctrl-c | cancel |

## Behaviour worth knowing

`gcb <name>` resolves a name the way you would expect to be able to say it: an
existing branch is switched to, a name only one remote has is checked out to
track that remote, and a name nothing answers to is created. Picking a
remote-tracking branch from the list switches to the local branch of that name
if there already is one, rather than creating a second one beside it.

`gcp <ref>` decides by what the argument names: a branch opens the commit
picker, anything else is cherry-picked directly. Commits already applied here
under a different hash are left out of the list, since picking them again would
do nothing. Multiple commits are applied oldest first, whatever order they were
marked in. Backing out of the commit list returns to the branch list rather than
quitting, because it usually means the wrong branch was chosen.

`ga` lists only what is not yet staged. Its picker matches on the path alone, so
a search cannot be satisfied by a status letter. Untracked files are listed
individually rather than collapsed into their directory, and their preview shows
the content that would be added.

## Design

Reading and writing are split, and for a reason.

**Reads go through libgit2, in process.** Status, branches, history and patches
are all read with `git2`, so opening a picker spawns nothing at all. The
alternative — parsing the output of `git status` and `git log` — is what makes
tools like this brittle: filenames that need quoting, formats that change, one
process per question asked.

**Writes go through git itself.** `git add`, `git switch` and `git cherry-pick`
are run as commands. libgit2 can do all three, but not the parts that matter
when something goes wrong: it does not run hooks and does not write the
sequencer state that lets `git cherry-pick --continue` finish a conflicted pick.
A cherry-pick that stops on a conflict leaves the repository in exactly the
state git would have left it in, and the usual commands carry on from there.

**The picker is part of the program.** It is drawn with `ratatui` in an inline
viewport, so scrollback survives and the list disappears when it is done, and it
matches with `nucleo-matcher`. Drawing goes to the terminal device rather than
stdout, so redirecting output does not disturb it, and the terminal is restored
by a `Drop` implementation, so no way out of the picker — including a panic —
can leave it in raw mode.

The layers are kept apart: `repo` knows git but not the terminal, `picker` knows
the terminal but not git, and `rows` is the only place that knows both.
