// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadmeFormat {
    Markdown,
    PlainText,
}

pub struct About {
    pub repository: super::repository::Info,
    pub bytes: Vec<u8>,
    pub format: ReadmeFormat,
}

impl About {
    pub fn load(root: &Path, name: &str) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let commit = super::revision::commit(&repository, &crate::Revision::Head)?;
        let tree = commit.tree().map_err(super::Error::from_git)?;
        let (path, blob) = ["README.md", "README"]
            .into_iter()
            .find_map(|path| {
                tree.get_path(Path::new(path))
                    .ok()
                    .filter(|entry| entry.kind() == Some(git2::ObjectType::Blob))
                    .and_then(|entry| repository.find_blob(entry.id()).ok())
                    .map(|blob| (path, blob))
            })
            .ok_or(super::Error::NotFound)?;
        Ok(Self {
            repository: info,
            bytes: blob.content().to_vec(),
            format: if path.ends_with(".md") {
                ReadmeFormat::Markdown
            } else {
                ReadmeFormat::PlainText
            },
        })
    }
}
