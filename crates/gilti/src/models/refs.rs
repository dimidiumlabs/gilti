// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

pub struct Refs {
    pub repository: super::repository::Info,
    pub branches: Vec<Branch>,
    pub tags: Vec<Tag>,
}

pub struct Branch {
    pub name: String,
    pub reference: String,
    pub subject: String,
    pub author: String,
    pub timestamp: i64,
}

pub struct Tag {
    pub name: String,
    pub reference: String,
    pub target: String,
    pub downloadable: bool,
    pub author: String,
    pub timestamp: i64,
}

impl Refs {
    pub fn load(root: &Path, name: &str) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        repository
            .head()
            .and_then(|head| head.peel_to_commit())
            .map_err(super::Error::from_git)?;
        let info = super::repository::info(&repository, name);
        let mut branches = Vec::new();
        let references = repository.references().map_err(super::Error::from_git)?;
        for reference in references {
            let reference = reference.map_err(super::Error::from_git)?;
            let Ok(reference_name) = reference.name() else {
                continue;
            };
            if !reference_name.starts_with("refs/heads/") {
                continue;
            }
            let commit = match reference.peel_to_commit() {
                Ok(commit) => commit,
                Err(_) => continue,
            };
            branches.push(Branch {
                name: reference_name["refs/heads/".len()..].to_owned(),
                reference: reference_name.to_owned(),
                subject: first_line(commit.message_bytes()),
                author: signature_name(&commit.author()),
                timestamp: commit.time().seconds(),
            });
        }
        branches.sort_by(|left, right| left.reference.cmp(&right.reference));

        let mut tags = Vec::new();
        let references = repository.references().map_err(super::Error::from_git)?;
        for reference in references {
            let reference = reference.map_err(super::Error::from_git)?;
            let Ok(reference_name) = reference.name() else {
                continue;
            };
            if !reference_name.starts_with("refs/tags/") {
                continue;
            }
            let object = match reference
                .resolve()
                .and_then(|reference| {
                    reference
                        .target()
                        .ok_or_else(|| git2::Error::from_str("reference has no target"))
                })
                .and_then(|oid| repository.find_object(oid, None))
            {
                Ok(object) => object,
                Err(_) => continue,
            };
            let (author, timestamp) = if let Some(tag) = object.as_tag() {
                let tagger = tag.tagger();
                (
                    tagger.as_ref().map_or_else(String::new, signature_name),
                    tagger.as_ref().map_or(0, |tagger| tagger.when().seconds()),
                )
            } else if let Some(commit) = object.as_commit() {
                (signature_name(&commit.author()), commit.time().seconds())
            } else {
                (String::new(), 0)
            };
            let target = object
                .as_tag()
                .map_or_else(|| object.id(), git2::Tag::target_id);
            let downloadable = object.peel_to_commit().is_ok();
            tags.push(Tag {
                name: reference_name["refs/tags/".len()..].to_owned(),
                reference: reference_name.to_owned(),
                target: target.to_string(),
                downloadable,
                author,
                timestamp,
            });
        }
        tags.sort_by(|left, right| {
            right
                .timestamp
                .cmp(&left.timestamp)
                .then_with(|| left.reference.cmp(&right.reference))
        });
        Ok(Self {
            repository: info,
            branches,
            tags,
        })
    }
}

fn signature_name(signature: &git2::Signature<'_>) -> String {
    signature
        .name()
        .map(str::to_owned)
        .unwrap_or_else(|_| String::from_utf8_lossy(signature.name_bytes()).into_owned())
}

fn first_line(message: &[u8]) -> String {
    String::from_utf8_lossy(
        message
            .split(|byte| *byte == b'\n')
            .next()
            .unwrap_or_default(),
    )
    .into_owned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn loads_local_branches_and_annotated_tags() {
        let root = std::env::temp_dir().join(format!("gilti-refs-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let repository = git2::Repository::init_bare(root.join("example.git")).unwrap();
        let signature = git2::Signature::new(
            "Example Author",
            "author@example.test",
            &git2::Time::new(1_700_000_000, 0),
        )
        .unwrap();
        let tree_oid = repository.treebuilder(None).unwrap().write().unwrap();
        let tree = repository.find_tree(tree_oid).unwrap();
        let commit_oid = repository
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "Subject line\n\nBody",
                &tree,
                &[],
            )
            .unwrap();
        repository.set_head("refs/heads/main").unwrap();
        repository
            .reference("refs/heads/feature/x", commit_oid, false, "test")
            .unwrap();
        let commit = repository.find_object(commit_oid, None).unwrap();
        repository
            .tag("v1.0", &commit, &signature, "Release", false)
            .unwrap();
        drop(commit);
        drop(tree);
        drop(repository);

        let refs = super::Refs::load(&root, "example").unwrap();
        assert_eq!(
            refs.branches
                .iter()
                .map(|branch| branch.name.as_str())
                .collect::<Vec<_>>(),
            ["feature/x", "main"]
        );
        assert!(
            refs.branches
                .iter()
                .all(|branch| branch.subject == "Subject line")
        );
        assert_eq!(refs.tags.len(), 1);
        assert_eq!(refs.tags[0].name, "v1.0");
        assert_eq!(refs.tags[0].author, "Example Author");
        assert!(refs.tags[0].downloadable);

        std::fs::remove_dir_all(root).unwrap();
    }
}
