// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

pub struct Info {
    pub name: String,
    pub description: String,
    pub has_readme: bool,
}

pub(crate) fn info(repository: &git2::Repository, name: &str) -> Info {
    let description = std::fs::read_to_string(repository.path().join("description"))
        .ok()
        .map(|description| description.trim().to_owned())
        .filter(|description| !description.is_empty())
        .unwrap_or_else(|| "[no description]".to_owned());
    let has_readme = repository
        .head()
        .and_then(|head| head.peel_to_tree())
        .ok()
        .is_some_and(|tree| {
            ["README.md", "README"].iter().any(|path| {
                tree.get_path(Path::new(path))
                    .is_ok_and(|entry| entry.kind() == Some(git2::ObjectType::Blob))
            })
        });
    Info {
        name: name.to_owned(),
        description,
        has_readme,
    }
}

pub(crate) fn open(root: &Path, name: &str) -> Result<git2::Repository, super::Error> {
    git2::Repository::open_bare(path(root, name)?).map_err(super::Error::from_git)
}

pub fn path(root: &Path, name: &str) -> Result<PathBuf, super::Error> {
    let root = root
        .canonicalize()
        .map_err(|error| super::Error::Internal(error.to_string()))?;
    let path = repository_path(&root, name)
        .canonicalize()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                super::Error::NotFound
            } else {
                super::Error::Internal(error.to_string())
            }
        })?;
    if !path.starts_with(&root) || !path.is_dir() {
        return Err(super::Error::NotFound);
    }
    Ok(path)
}

fn repository_path(root: &Path, name: &str) -> PathBuf {
    let (parent, leaf) = name.rsplit_once('/').unwrap_or(("", name));
    root.join(parent).join(format!("{leaf}.git"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn appends_exactly_one_storage_suffix() {
        let root = std::path::Path::new("/repositories");
        assert_eq!(
            super::repository_path(root, "group/project"),
            root.join("group/project.git")
        );
        assert_eq!(
            super::repository_path(root, "group/project.git"),
            root.join("group/project.git.git")
        );
    }

    #[test]
    fn resolved_path_cannot_escape_the_repository_root() {
        let parent =
            std::env::temp_dir().join(format!("gilti-repository-path-test-{}", std::process::id()));
        let root = parent.join("repositories");
        let outside = parent.join("outside.git");
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&root).unwrap();
        git2::Repository::init_bare(&outside).unwrap();
        assert!(matches!(
            super::path(&root, "../outside"),
            Err(crate::Error::NotFound)
        ));
        std::fs::remove_dir_all(parent).unwrap();
    }
}
