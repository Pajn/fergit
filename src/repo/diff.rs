use git2::{Diff, DiffFormat, DiffOptions, Oid};

use crate::error::Result;
use crate::repo::Repo;
use crate::repo::status::ChangedFile;

/// The role a line plays in a patch, so the terminal layer can style it
/// without reading the text back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Commit metadata above the patch itself.
    Meta,
    /// `diff --git a/... b/...` and the lines under it.
    FileHeader,
    /// `@@ ... @@`
    Hunk,
    Addition,
    Deletion,
    Context,
}

#[derive(Debug, Clone)]
pub struct PatchLine {
    pub kind: LineKind,
    pub text: String,
}

impl PatchLine {
    fn new(kind: LineKind, text: impl Into<String>) -> Self {
        PatchLine {
            kind,
            text: text.into(),
        }
    }
}

impl Repo {
    /// The unstaged changes to one file, as a patch.
    pub fn file_patch(&self, file: &ChangedFile) -> Result<Vec<PatchLine>> {
        let mut opts = DiffOptions::new();
        opts.pathspec(&file.path)
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            // Without this an untracked file is a one-line "new file" entry,
            // which tells you nothing about what you are about to stage.
            .show_untracked_content(true)
            .include_typechange(true)
            .context_lines(self.diff_context());

        let diff = self.inner.diff_index_to_workdir(None, Some(&mut opts))?;
        let lines = render(&diff)?;
        if lines.is_empty() {
            return Ok(vec![PatchLine::new(
                LineKind::Meta,
                "No unstaged changes to show.",
            )]);
        }
        Ok(lines)
    }

    /// A commit, the way `git show` presents it: metadata then the patch.
    pub fn commit_patch(&self, id: Oid) -> Result<Vec<PatchLine>> {
        let commit = self.inner.find_commit(id)?;
        let mut out = vec![
            PatchLine::new(LineKind::Meta, format!("commit {}", commit.id())),
            PatchLine::new(
                LineKind::Meta,
                format!(
                    "Author: {} <{}>",
                    commit.author().name().unwrap_or(""),
                    commit.author().email().unwrap_or("")
                ),
            ),
        ];
        out.push(PatchLine::new(LineKind::Meta, String::new()));
        for line in commit.message().unwrap_or("").lines() {
            out.push(PatchLine::new(LineKind::Meta, format!("    {line}")));
        }
        out.push(PatchLine::new(LineKind::Meta, String::new()));

        let new_tree = commit.tree()?;
        let old_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let mut opts = DiffOptions::new();
        opts.context_lines(self.diff_context());
        let diff =
            self.inner
                .diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;
        out.extend(render(&diff)?);
        Ok(out)
    }

    fn diff_context(&self) -> u32 {
        self.config_string("diff.context")
            .and_then(|v| v.parse().ok())
            .unwrap_or(3)
    }
}

/// Walk a diff into lines we can style. libgit2 hands us the same text git
/// would print, one callback per line, with a marker for what it is.
fn render(diff: &Diff<'_>) -> Result<Vec<PatchLine>> {
    let mut out = Vec::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let text = String::from_utf8_lossy(line.content());
        let text = text.trim_end_matches('\n');
        match line.origin() {
            '+' => out.push(PatchLine::new(LineKind::Addition, format!("+{text}"))),
            '-' => out.push(PatchLine::new(LineKind::Deletion, format!("-{text}"))),
            ' ' => out.push(PatchLine::new(LineKind::Context, format!(" {text}"))),
            // File and hunk headers arrive as blocks of several lines.
            'F' => out.extend(
                text.lines()
                    .map(|l| PatchLine::new(LineKind::FileHeader, l)),
            ),
            'H' => out.extend(text.lines().map(|l| PatchLine::new(LineKind::Hunk, l))),
            'B' => out.push(PatchLine::new(LineKind::Meta, "Binary file")),
            // Markers like "\ No newline at end of file".
            _ => out.push(PatchLine::new(LineKind::Meta, text)),
        }
        true
    })?;
    Ok(out)
}
