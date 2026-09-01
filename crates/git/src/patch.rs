// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

pub struct Patch {
    pub repository_path: PathBuf,
    pub old_oid: String,
    pub new_oid: String,
}

impl Patch {
    pub fn load(
        root: &Path,
        name: &str,
        old: &crate::Revision,
        new: &crate::Revision,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let old = super::revision::commit(&repository, old)?;
        let new = super::revision::commit(&repository, new)?;
        Ok(Self {
            repository_path: repository.path().to_owned(),
            old_oid: old.id().to_string(),
            new_oid: new.id().to_string(),
        })
    }
}
