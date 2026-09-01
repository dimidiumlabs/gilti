// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

pub struct Archive {
    pub repository_path: PathBuf,
    pub oid: String,
    pub prefix: String,
}

impl Archive {
    pub fn load(
        root: &Path,
        name: &str,
        revision: &crate::Revision,
        path: Option<&str>,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let commit = super::revision::commit(&repository, revision)?;
        if let Some(path) = path {
            commit
                .tree()
                .and_then(|tree| tree.get_path(Path::new(path)))
                .map_err(|_| super::Error::NotFound)?;
        }
        let repository_path = repository.path().to_owned();
        let prefix = name
            .rsplit('/')
            .next()
            .unwrap_or(name)
            .strip_suffix(".git")
            .unwrap_or_else(|| name.rsplit('/').next().unwrap_or(name))
            .to_owned();
        Ok(Self {
            repository_path,
            oid: commit.id().to_string(),
            prefix,
        })
    }
}
