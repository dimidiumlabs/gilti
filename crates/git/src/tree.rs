// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

pub struct Tree {
    pub repository: super::repository::Info,
    pub revision: String,
    pub path: Option<String>,
    pub content: Content,
}

pub enum Content {
    Directory {
        oid: String,
        entries: Vec<Entry>,
    },
    Blob {
        oid: String,
        bytes: Vec<u8>,
        binary: bool,
    },
}

pub struct Entry {
    pub mode: u32,
    pub name: String,
    pub path: String,
    pub oid: String,
    pub kind: Kind,
    pub size: usize,
    pub symlink_target: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Tree,
    Blob,
    Submodule,
}

impl Tree {
    pub fn load(
        root: &Path,
        name: &str,
        revision: crate::Revision,
        path: Option<String>,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let selector = super::revision::selector(&revision);
        let commit = super::revision::commit(&repository, &revision)?;
        let root_tree = commit.tree().map_err(super::Error::from_git)?;
        let content = match path.as_deref() {
            None => directory(&repository, &root_tree, "")?,
            Some(path) => {
                let entry = root_tree
                    .get_path(Path::new(path))
                    .map_err(super::Error::from_git)?;
                match entry.kind() {
                    Some(git2::ObjectType::Tree) => {
                        let tree = repository
                            .find_tree(entry.id())
                            .map_err(super::Error::from_git)?;
                        directory(&repository, &tree, path)?
                    }
                    Some(git2::ObjectType::Blob) => {
                        let blob = repository
                            .find_blob(entry.id())
                            .map_err(super::Error::from_git)?;
                        let bytes = blob.content().to_vec();
                        let binary = bytes.iter().take(8000).any(|byte| *byte == 0);
                        Content::Blob {
                            oid: entry.id().to_string(),
                            bytes,
                            binary,
                        }
                    }
                    _ => return Err(super::Error::NotFound),
                }
            }
        };
        Ok(Self {
            repository: info,
            revision: selector,
            path,
            content,
        })
    }
}

fn directory(
    repository: &git2::Repository,
    tree: &git2::Tree<'_>,
    base: &str,
) -> Result<Content, super::Error> {
    let odb = repository.odb().map_err(super::Error::from_git)?;
    let mut entries = Vec::with_capacity(tree.len());
    for entry in tree {
        let name = String::from_utf8_lossy(entry.name_bytes()).into_owned();
        let path = if base.is_empty() {
            name.clone()
        } else {
            format!("{base}/{name}")
        };
        let kind = match entry.kind() {
            Some(git2::ObjectType::Tree) => Kind::Tree,
            Some(git2::ObjectType::Commit) => Kind::Submodule,
            Some(git2::ObjectType::Blob) => Kind::Blob,
            _ => continue,
        };
        let size = if kind == Kind::Submodule {
            0
        } else {
            odb.read_header(entry.id())
                .map_err(super::Error::from_git)?
                .0
        };
        let symlink_target = if entry.filemode_raw() == 0o120000 {
            repository
                .find_blob(entry.id())
                .ok()
                .map(|blob| String::from_utf8_lossy(blob.content()).into_owned())
        } else {
            None
        };
        entries.push(Entry {
            mode: entry.filemode_raw() as u32,
            name,
            path,
            oid: entry.id().to_string(),
            kind,
            size,
            symlink_target,
        });
    }
    entries.sort_by(|left, right| {
        let left_group = usize::from(left.kind != Kind::Tree);
        let right_group = usize::from(right.kind != Kind::Tree);
        left_group
            .cmp(&right_group)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(Content::Directory {
        oid: tree.id().to_string(),
        entries,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn loads_directories_blobs_modes_and_symlinks() {
        let root = std::env::temp_dir().join(format!("gilti-tree-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let repository = git2::Repository::init_bare(root.join("example.git")).unwrap();
        let odb = repository.odb().unwrap();
        let readme = odb.write(git2::ObjectType::Blob, b"hello\n").unwrap();
        let executable = odb.write(git2::ObjectType::Blob, b"#!/bin/sh\n").unwrap();
        let symlink = odb.write(git2::ObjectType::Blob, b"README.md").unwrap();
        let nested = odb.write(git2::ObjectType::Blob, b"nested\n").unwrap();
        let subtree = {
            let mut tree = repository.treebuilder(None).unwrap();
            tree.insert("file.txt", nested, 0o100644).unwrap();
            tree.write().unwrap()
        };
        let tree_oid = {
            let mut tree = repository.treebuilder(None).unwrap();
            tree.insert("README.md", readme, 0o100644).unwrap();
            tree.insert("run", executable, 0o100755).unwrap();
            tree.insert("link", symlink, 0o120000).unwrap();
            tree.insert("dir", subtree, 0o040000).unwrap();
            tree.write().unwrap()
        };
        let tree = repository.find_tree(tree_oid).unwrap();
        let signature = git2::Signature::now("Author", "author@example.test").unwrap();
        repository
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "Tree fixture",
                &tree,
                &[],
            )
            .unwrap();
        repository.set_head("refs/heads/main").unwrap();
        drop(tree);
        drop(odb);
        drop(repository);

        let root_tree = super::Tree::load(&root, "example", crate::Revision::Head, None).unwrap();
        let super::Content::Directory { entries, .. } = root_tree.content else {
            panic!("expected directory")
        };
        assert_eq!(entries.len(), 4);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["dir", "README.md", "link", "run"],
        );
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.name == "run")
                .unwrap()
                .mode,
            0o100755
        );
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.name == "link")
                .unwrap()
                .symlink_target
                .as_deref(),
            Some("README.md")
        );
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.name == "dir")
                .unwrap()
                .kind,
            super::Kind::Tree
        );

        let blob = super::Tree::load(
            &root,
            "example",
            crate::Revision::Head,
            Some("dir/file.txt".to_owned()),
        )
        .unwrap();
        let super::Content::Blob { bytes, binary, .. } = blob.content else {
            panic!("expected blob")
        };
        assert_eq!(bytes, b"nested\n");
        assert!(!binary);
        assert!(matches!(
            super::Tree::load(
                &root,
                "example",
                crate::Revision::Head,
                Some("missing".to_owned())
            ),
            Err(crate::Error::NotFound)
        ));
        assert!(matches!(
            super::Tree::load(
                &root,
                "example",
                crate::Revision::Commit(readme.to_string()),
                None
            ),
            Err(crate::Error::NotFound)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
