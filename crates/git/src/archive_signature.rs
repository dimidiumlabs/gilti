// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

pub struct ArchiveSignature {
    pub bytes: Vec<u8>,
    pub oid: String,
    pub filename: String,
}

impl ArchiveSignature {
    pub fn load(
        root: &Path,
        name: &str,
        revision: crate::Revision,
        format: &str,
    ) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let commit = super::revision::commit(&repository, &revision)?;
        let notes_ref = format!("refs/notes/signatures/{format}");
        let note = repository
            .find_note(Some(&notes_ref), commit.id())
            .map_err(super::Error::from_git)?;
        Ok(Self {
            bytes: note.message_bytes().to_vec(),
            oid: note.id().to_string(),
            filename: format!("{}.{}.asc", name.rsplit('/').next().unwrap_or(name), format),
        })
    }
}
