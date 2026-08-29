// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

#[derive(Clone, Copy)]
pub struct Options {
    pub context: u32,
    pub ignore_whitespace: bool,
}

pub struct Diff {
    pub repository: super::repository::Info,
    pub old_revision: Option<String>,
    pub new_revision: String,
    pub old_oid: Option<String>,
    pub new_oid: String,
    pub files: Vec<File>,
    pub additions: usize,
    pub deletions: usize,
}

pub struct File {
    pub status: git2::Delta,
    pub old_path: String,
    pub new_path: String,
    pub old_oid: Option<String>,
    pub new_oid: Option<String>,
    pub old_mode: u32,
    pub new_mode: u32,
    pub old_size: u64,
    pub new_size: u64,
    pub binary: bool,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<Hunk>,
}

pub struct Hunk {
    pub header: String,
    pub lines: Vec<Line>,
}

pub struct Line {
    pub origin: char,
    pub content: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

impl Diff {
    pub fn load(
        root: &Path,
        name: &str,
        old_revision: Option<crate::router::Revision>,
        new_revision: crate::router::Revision,
        path: Option<String>,
        options: Options,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let new_commit = super::revision::commit(&repository, &new_revision)?;
        let old_commit = old_revision
            .as_ref()
            .map(|revision| super::revision::commit(&repository, revision))
            .transpose()?;
        let old_tree = old_commit
            .as_ref()
            .map(git2::Commit::tree)
            .transpose()
            .map_err(super::Error::from_git)?;
        let new_tree = new_commit.tree().map_err(super::Error::from_git)?;
        let mut diff_options = git2::DiffOptions::new();
        diff_options.context_lines(options.context);
        if options.ignore_whitespace {
            diff_options.ignore_whitespace(true);
        }
        if let Some(path) = &path {
            diff_options.pathspec(path).disable_pathspec_match(true);
        }
        let mut diff = repository
            .diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut diff_options))
            .map_err(super::Error::from_git)?;
        diff.find_similar(None).map_err(super::Error::from_git)?;
        let stats = diff.stats().map_err(super::Error::from_git)?;
        let mut files = Vec::with_capacity(diff.deltas().len());
        for (index, delta) in diff.deltas().enumerate() {
            let old_file = delta.old_file();
            let new_file = delta.new_file();
            let patch = git2::Patch::from_diff(&diff, index).map_err(super::Error::from_git)?;
            let (additions, deletions, hunks) = if let Some(patch) = patch {
                let (_, additions, deletions) =
                    patch.line_stats().map_err(super::Error::from_git)?;
                let mut hunks = Vec::with_capacity(patch.num_hunks());
                for hunk_index in 0..patch.num_hunks() {
                    let (hunk, lines) = patch.hunk(hunk_index).map_err(super::Error::from_git)?;
                    let mut result = Hunk {
                        header: String::from_utf8_lossy(hunk.header())
                            .trim_end_matches('\n')
                            .to_owned(),
                        lines: Vec::with_capacity(lines),
                    };
                    for line_index in 0..lines {
                        let line = patch
                            .line_in_hunk(hunk_index, line_index)
                            .map_err(super::Error::from_git)?;
                        result.lines.push(Line {
                            origin: line.origin(),
                            content: String::from_utf8_lossy(line.content())
                                .trim_end_matches('\n')
                                .to_owned(),
                            old_line: line.old_lineno(),
                            new_line: line.new_lineno(),
                        });
                    }
                    hunks.push(result);
                }
                (additions, deletions, hunks)
            } else {
                (0, 0, Vec::new())
            };
            files.push(File {
                status: delta.status(),
                old_path: path_string(old_file.path_bytes()),
                new_path: path_string(new_file.path_bytes()),
                old_oid: oid(old_file.id()),
                new_oid: oid(new_file.id()),
                old_mode: old_file.mode().into(),
                new_mode: new_file.mode().into(),
                old_size: old_file.size(),
                new_size: new_file.size(),
                binary: old_file.is_binary() || new_file.is_binary(),
                additions,
                deletions,
                hunks,
            });
        }
        Ok(Self {
            repository: info,
            old_revision: old_revision.as_ref().map(super::revision::selector),
            new_revision: super::revision::selector(&new_revision),
            old_oid: old_commit.map(|commit| commit.id().to_string()),
            new_oid: new_commit.id().to_string(),
            files,
            additions: stats.insertions(),
            deletions: stats.deletions(),
        })
    }
}

fn path_string(path: Option<&[u8]>) -> String {
    path.map_or_else(String::new, |path| {
        String::from_utf8_lossy(path).into_owned()
    })
}

fn oid(oid: git2::Oid) -> Option<String> {
    (!oid.is_zero()).then(|| oid.to_string())
}
