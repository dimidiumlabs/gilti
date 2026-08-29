// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

pub struct Commit {
    pub repository: super::repository::Info,
    pub revision: String,
    pub oid: String,
    pub tree: String,
    pub author: Identity,
    pub committer: Identity,
    pub subject: String,
    pub message: String,
    pub parents: Vec<String>,
    pub decorations: Vec<Decoration>,
    pub notes: Option<String>,
    pub diff: Option<super::diff::Diff>,
}

pub struct Identity {
    pub name: String,
    pub email: String,
    pub timestamp: i64,
    pub offset_minutes: i32,
}

pub struct Decoration {
    pub label: String,
    pub reference: Option<String>,
}

impl Commit {
    pub fn load(
        root: &Path,
        name: &str,
        revision: crate::router::Revision,
        diff_options: super::diff::Options,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let commit = super::revision::commit(&repository, &revision)?;
        let oid = commit.id();
        let parents = commit
            .parent_ids()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        let author = identity(commit.author());
        let committer = identity(commit.committer());
        let message = String::from_utf8_lossy(commit.message_bytes());
        let subject =
            String::from_utf8_lossy(commit.summary_bytes().unwrap_or_default()).into_owned();
        let body = message
            .split_once('\n')
            .map_or("", |(_, body)| body.trim_start_matches('\n'))
            .to_owned();
        let notes = repository
            .find_note(None, oid)
            .ok()
            .map(|note| String::from_utf8_lossy(note.message_bytes()).into_owned());
        let refs = super::refs::Refs::load(root, name)?;
        let mut decorations = Vec::new();
        if repository
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok())
            .is_some_and(|head| head.id() == oid)
        {
            decorations.push(Decoration {
                label: "HEAD".to_owned(),
                reference: None,
            });
        }
        for branch in refs.branches {
            if repository
                .find_reference(&branch.reference)
                .ok()
                .and_then(|reference| reference.peel_to_commit().ok())
                .is_some_and(|target| target.id() == oid)
            {
                decorations.push(Decoration {
                    label: branch.name,
                    reference: Some(branch.reference),
                });
            }
        }
        for tag in refs.tags {
            if repository
                .find_reference(&tag.reference)
                .ok()
                .and_then(|reference| reference.peel_to_commit().ok())
                .is_some_and(|target| target.id() == oid)
            {
                decorations.push(Decoration {
                    label: tag.name,
                    reference: Some(tag.reference),
                });
            }
        }
        let old_revision = (parents.len() < 3)
            .then(|| parents.first().cloned())
            .flatten()
            .map(crate::router::Revision::Commit);
        let show_diff = parents.len() < 3;
        let revision_name = super::revision::selector(&revision);
        let tree = commit.tree_id().to_string();
        drop(commit);
        drop(repository);
        let diff = show_diff
            .then(|| {
                super::diff::Diff::load(
                    root,
                    name,
                    old_revision,
                    revision.clone(),
                    None,
                    diff_options,
                )
            })
            .transpose()?;
        Ok(Self {
            repository: info,
            revision: revision_name,
            oid: oid.to_string(),
            tree,
            author,
            committer,
            subject,
            message: body,
            parents,
            decorations,
            notes,
            diff,
        })
    }
}

fn identity(signature: git2::Signature<'_>) -> Identity {
    Identity {
        name: signature
            .name()
            .map(str::to_owned)
            .unwrap_or_else(|_| String::from_utf8_lossy(signature.name_bytes()).into_owned()),
        email: signature
            .email()
            .map(str::to_owned)
            .unwrap_or_else(|_| String::from_utf8_lossy(signature.email_bytes()).into_owned()),
        timestamp: signature.when().seconds(),
        offset_minutes: signature.when().offset_minutes(),
    }
}
