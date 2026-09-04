// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Format {
    Tar,
    TarGzip,
    TarBzip2,
    TarXz,
    TarZstd,
    Zip,
}

impl Format {
    pub const ALL: [Self; 6] = [
        Self::Tar,
        Self::TarGzip,
        Self::TarBzip2,
        Self::TarXz,
        Self::TarZstd,
        Self::Zip,
    ];

    /// Parses the wire-format query value. A missing value retains the legacy
    /// `tar.gz` protocol fallback; configured format exposure is enforced by the caller.
    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value.unwrap_or("tar.gz") {
            "tar" => Some(Self::Tar),
            "tar.gz" => Some(Self::TarGzip),
            "tar.bz2" => Some(Self::TarBzip2),
            "tar.xz" => Some(Self::TarXz),
            "tar.zst" => Some(Self::TarZstd),
            "zip" => Some(Self::Zip),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tar => "tar",
            Self::TarGzip => "tar.gz",
            Self::TarBzip2 => "tar.bz2",
            Self::TarXz => "tar.xz",
            Self::TarZstd => "tar.zst",
            Self::Zip => "zip",
        }
    }

    pub(crate) const fn git_format(self) -> &'static str {
        if matches!(self, Self::Zip) {
            "zip"
        } else {
            "tar"
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

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

#[cfg(test)]
mod tests {
    use super::Format;

    #[test]
    fn parses_supported_archive_formats() {
        for format in Format::ALL {
            assert_eq!(Format::parse(Some(format.as_str())), Some(format));
        }
        assert_eq!(Format::parse(None), Some(Format::TarGzip));
        assert_eq!(Format::parse(Some("tar.lz")), None);
    }
}
