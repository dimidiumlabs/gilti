// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::Path;

pub struct Overview {
    pub repository: super::repository::Info,
    pub empty: bool,
    pub branches: Vec<super::refs::Branch>,
    pub tags: Vec<super::refs::Tag>,
    pub commits: Vec<Commit>,
}

pub struct Commit {
    pub oid: String,
    pub subject: String,
    pub author: String,
    pub timestamp: i64,
    pub files: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub decorations: Vec<Decoration>,
}

pub struct Decoration {
    pub label: String,
    pub reference: Option<String>,
    pub tag: bool,
}

impl Overview {
    pub fn load(
        root: &Path,
        name: &str,
        max_refs: usize,
        max_commits: usize,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let head = match repository.head().and_then(|head| head.peel_to_commit()) {
            Ok(head) => head,
            Err(error) if error.code() == git2::ErrorCode::UnbornBranch => {
                return Ok(Self {
                    repository: info,
                    empty: true,
                    branches: Vec::new(),
                    tags: Vec::new(),
                    commits: Vec::new(),
                });
            }
            Err(error) => return Err(super::Error::from_git(error)),
        };
        let refs = super::refs::Refs::load(root, name)?;
        let mut decorations = HashMap::<git2::Oid, Vec<Decoration>>::new();
        decorations.entry(head.id()).or_default().push(Decoration {
            label: "HEAD".to_owned(),
            reference: None,
            tag: false,
        });
        for branch in &refs.branches {
            if let Ok(oid) = repository.refname_to_id(&branch.reference) {
                decorations.entry(oid).or_default().push(Decoration {
                    label: branch.name.clone(),
                    reference: Some(branch.reference.clone()),
                    tag: false,
                });
            }
        }
        for tag in &refs.tags {
            if let Ok(reference) = repository.find_reference(&tag.reference)
                && let Ok(commit) = reference.peel_to_commit()
            {
                decorations
                    .entry(commit.id())
                    .or_default()
                    .push(Decoration {
                        label: tag.name.clone(),
                        reference: Some(tag.reference.clone()),
                        tag: true,
                    });
            }
        }
        let mut walk = repository.revwalk().map_err(super::Error::from_git)?;
        walk.push(head.id()).map_err(super::Error::from_git)?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
            .map_err(super::Error::from_git)?;
        let mut commits = Vec::new();
        for oid in walk.take(max_commits) {
            let oid = oid.map_err(super::Error::from_git)?;
            let commit = repository
                .find_commit(oid)
                .map_err(super::Error::from_git)?;
            let tree = commit.tree().map_err(super::Error::from_git)?;
            let parent_tree = commit.parent(0).ok().and_then(|parent| parent.tree().ok());
            let diff = repository
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
                .map_err(super::Error::from_git)?;
            let stats = diff.stats().map_err(super::Error::from_git)?;
            commits.push(Commit {
                oid: oid.to_string(),
                subject: String::from_utf8_lossy(commit.summary_bytes().unwrap_or_default())
                    .into_owned(),
                author: signature_name(&commit.author()),
                timestamp: commit.time().seconds(),
                files: stats.files_changed(),
                insertions: stats.insertions(),
                deletions: stats.deletions(),
                decorations: decorations.remove(&oid).unwrap_or_default(),
            });
        }
        Ok(Self {
            repository: info,
            empty: false,
            branches: refs.branches.into_iter().take(max_refs).collect(),
            tags: refs.tags.into_iter().take(max_refs).collect(),
            commits,
        })
    }
}

fn signature_name(signature: &git2::Signature<'_>) -> String {
    signature
        .name()
        .map(str::to_owned)
        .unwrap_or_else(|_| String::from_utf8_lossy(signature.name_bytes()).into_owned())
}
