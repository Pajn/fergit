//! How repository objects are presented in a picker.
//!
//! Kept apart from both layers: the repository types stay free of terminal
//! concerns, and the picker stays free of any knowledge of git.

use crate::format::{self, relative_time};
use crate::picker::theme;
use crate::picker::{Item, Row};
use crate::repo::branch::{Branch, BranchKind};
use crate::repo::commit::CommitSummary;
use crate::repo::status::ChangedFile;

/// A changed file together with the path as the user should see it.
pub struct FileRow {
    pub file: ChangedFile,
    pub display: String,
}

impl Item for FileRow {
    fn row(&self) -> Row {
        let code = self.file.code();
        let mut chars = code.chars();
        let (index, worktree) = (chars.next().unwrap_or(' '), chars.next().unwrap_or(' '));
        let (index_style, worktree_style) = theme::status_code(index, worktree);

        let row = Row::new(&self.display)
            .prefix(index.to_string(), index_style)
            .prefix(format!("{worktree} "), worktree_style);

        match &self.file.origin {
            Some(origin) => row.suffix(format!("  ← {}", origin.display()), theme::dim()),
            None => row,
        }
    }
}

impl Item for Branch {
    fn row(&self) -> Row {
        let marker = if self.is_head { "* " } else { "  " };
        let mut row = Row::new(&self.name).prefix(marker, theme::accent());
        match (self.kind, &self.upstream) {
            (BranchKind::Remote, _) => row = row.suffix("  remote", theme::dim()),
            (BranchKind::Local, Some(upstream)) => {
                row = row.suffix(format!("  → {upstream}"), theme::dim())
            }
            (BranchKind::Local, None) => {}
        }
        if !self.subject.is_empty() {
            row = row.suffix(format!("  {}", self.subject), theme::dim());
        }
        row.suffix(
            format!("  {}", relative_time(self.tip_time, format::now())),
            theme::dim(),
        )
    }
}

impl Item for CommitSummary {
    fn row(&self) -> Row {
        Row::new(&self.summary)
            .prefix(format!("{} ", self.short_id), theme::marked())
            .suffix(
                format!(
                    "  {}, {}",
                    self.author,
                    relative_time(self.time, format::now())
                ),
                theme::dim(),
            )
    }
}
