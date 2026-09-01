// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

pub struct Blame {
    pub repository: super::repository::Info,
    pub revision: String,
    pub path: String,
    pub oid: String,
    pub bytes: Vec<u8>,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

pub struct Hunk {
    pub oid: String,
    pub short_oid: String,
    pub start: usize,
    pub lines: usize,
    pub original_path: String,
    pub author: String,
    pub author_email: String,
    pub committer: String,
    pub committer_email: String,
    pub timestamp: i64,
    pub summary: String,
    pub parent: Option<String>,
}

impl Blame {
    pub fn load(
        root: &Path,
        name: &str,
        revision: crate::Revision,
        path: String,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let selector = super::revision::selector(&revision);
        let commit = super::revision::commit(&repository, &revision)?;
        let tree = commit.tree().map_err(super::Error::from_git)?;
        let entry = tree
            .get_path(Path::new(&path))
            .map_err(super::Error::from_git)?;
        if !matches!(entry.filemode_raw(), 0o100644 | 0o100755) {
            return Err(super::Error::NotFound);
        }
        let blob = repository
            .find_blob(entry.id())
            .map_err(super::Error::from_git)?;
        let bytes = blob.content().to_vec();
        let binary = bytes.iter().take(8000).any(|byte| *byte == 0);
        let mut hunks = Vec::new();
        if !binary {
            let mut options = git2::BlameOptions::new();
            options.newest_commit(commit.id());
            let blame = repository
                .blame_file(Path::new(&path), Some(&mut options))
                .map_err(super::Error::from_git)?;
            for hunk in blame.iter() {
                let oid = hunk.final_commit_id();
                let commit = repository
                    .find_commit(oid)
                    .map_err(super::Error::from_git)?;
                let author = hunk.final_signature().unwrap_or_else(|| commit.author());
                let committer = hunk.final_committer().unwrap_or_else(|| commit.committer());
                hunks.push(Hunk {
                    oid: oid.to_string(),
                    short_oid: oid.to_string().chars().take(7).collect(),
                    start: hunk.final_start_line(),
                    lines: hunk.lines_in_hunk(),
                    original_path: hunk
                        .path()
                        .map_or_else(|| path.clone(), |path| path.to_string_lossy().into_owned()),
                    author: signature_name(&author),
                    author_email: signature_email(&author),
                    committer: signature_name(&committer),
                    committer_email: signature_email(&committer),
                    timestamp: author.when().seconds(),
                    summary: String::from_utf8_lossy(
                        hunk.summary_bytes()
                            .unwrap_or(commit.summary_bytes().unwrap_or_default()),
                    )
                    .into_owned(),
                    parent: commit.parent_id(0).ok().map(|oid| oid.to_string()),
                });
            }
        }
        Ok(Self {
            repository: info,
            revision: selector,
            path,
            oid: entry.id().to_string(),
            bytes,
            binary,
            hunks,
        })
    }
}

fn signature_name(signature: &git2::Signature<'_>) -> String {
    signature
        .name()
        .map(str::to_owned)
        .unwrap_or_else(|_| String::from_utf8_lossy(signature.name_bytes()).into_owned())
}

fn signature_email(signature: &git2::Signature<'_>) -> String {
    signature
        .email()
        .map(str::to_owned)
        .unwrap_or_else(|_| String::from_utf8_lossy(signature.email_bytes()).into_owned())
}
