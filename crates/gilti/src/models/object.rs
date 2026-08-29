// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

pub struct RawObject {
    pub bytes: Vec<u8>,
    pub binary: bool,
}

impl RawObject {
    pub fn load(root: &Path, repository: &str, oid: &str) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, repository)?;
        let oid = git2::Oid::from_str_ext(oid, repository.object_format())
            .map_err(|_| super::Error::NotFound)?;
        let odb = repository.odb().map_err(super::Error::from_git)?;
        let object = odb.read(oid).map_err(super::Error::from_git)?;
        let bytes = object.data().to_vec();
        let binary = bytes.iter().take(8000).any(|byte| *byte == 0);
        Ok(Self { bytes, binary })
    }
}

#[cfg(test)]
mod tests {
    fn fixture() -> (std::path::PathBuf, git2::Repository) {
        let root = std::env::temp_dir().join(format!(
            "gilti-object-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("thread")
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let repository = git2::Repository::init_bare(root.join("example.git")).unwrap();
        (root, repository)
    }

    #[test]
    fn reads_raw_text_and_binary_objects_like_browser_blob() {
        let (root, repository) = fixture();
        let odb = repository.odb().unwrap();
        let text_oid = odb.write(git2::ObjectType::Blob, b"hello\n").unwrap();
        let binary_oid = odb
            .write(git2::ObjectType::Blob, b"binary\0payload")
            .unwrap();

        let text = super::RawObject::load(&root, "example", &text_oid.to_string()).unwrap();
        assert_eq!(text.bytes, b"hello\n");
        assert!(!text.binary);
        let binary = super::RawObject::load(&root, "example", &binary_oid.to_string()).unwrap();
        assert_eq!(binary.bytes, b"binary\0payload");
        assert!(binary.binary);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_sha256_objects_with_the_repository_format() {
        let root =
            std::env::temp_dir().join(format!("gilti-object-sha256-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut options = git2::RepositoryInitOptions::new();
        options.bare(true).object_format(git2::ObjectFormat::Sha256);
        let repository = git2::Repository::init_opts(root.join("example.git"), &options).unwrap();
        let oid = repository
            .odb()
            .unwrap()
            .write(git2::ObjectType::Blob, b"sha256\n")
            .unwrap();
        assert_eq!(oid.to_string().len(), 64);

        let object = super::RawObject::load(&root, "example", &oid.to_string()).unwrap();
        assert_eq!(object.bytes, b"sha256\n");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_object_is_not_found() {
        let (root, _repository) = fixture();
        assert!(matches!(
            super::RawObject::load(&root, "example", "0000000000000000000000000000000000000000"),
            Err(crate::models::Error::NotFound)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
