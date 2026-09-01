// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Safe local LFS object storage primitives.

use sha2::Digest;
use std::sync::atomic::{AtomicU64, Ordering};
static TEMPORARY_NONCE: AtomicU64 = AtomicU64::new(0);

pub const MAX_OBJECT_SIZE: usize = 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct LfsStore {
    objects: std::path::PathBuf,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreError {
    InvalidOid,
    NotFound,
    HashMismatch,
    Storage(String),
}

impl LfsStore {
    /// Opens a repository-owned LFS store after canonical containment validation.
    pub fn open(root: &std::path::Path, repo: &str) -> Result<Self, StoreError> {
        let root = std::fs::canonicalize(root).map_err(|_| StoreError::NotFound)?;
        let repository = std::fs::canonicalize(root.join(format!("{repo}.git")))
            .map_err(|_| StoreError::NotFound)?;
        if !repository.is_dir() || !repository.starts_with(&root) {
            return Err(StoreError::NotFound);
        }
        Ok(Self {
            objects: repository.join("lfs/objects"),
        })
    }
    pub fn present(&self, oid: &str, size: u64) -> Result<bool, StoreError> {
        let path = self.path(oid)?;
        Ok(std::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == size))
    }
    pub fn metadata(&self, oid: &str) -> Result<u64, StoreError> {
        let path = self.path(oid)?;
        std::fs::metadata(path)
            .ok()
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .ok_or(StoreError::NotFound)
    }
    pub fn read(&self, oid: &str) -> Result<Vec<u8>, StoreError> {
        std::fs::read(self.path(oid)?).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StoreError::NotFound
            } else {
                StoreError::Storage(e.to_string())
            }
        })
    }
    /// Verifies the object ID before atomically placing bytes in the store.
    pub fn write(&self, oid: &str, bytes: &[u8]) -> Result<(), StoreError> {
        if bytes.len() > MAX_OBJECT_SIZE {
            return Err(StoreError::Storage("object too large".into()));
        }
        if !verify_bytes(oid, bytes) {
            return Err(StoreError::HashMismatch);
        }
        let path = self.path(oid)?;
        let parent = path
            .parent()
            .ok_or_else(|| StoreError::Storage("invalid storage path".into()))?;
        std::fs::create_dir_all(parent).map_err(|e| StoreError::Storage(e.to_string()))?;
        let temporary = parent.join(format!(
            ".{oid}.{}.{}.tmp",
            std::process::id(),
            TEMPORARY_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&temporary, bytes).map_err(|e| StoreError::Storage(e.to_string()))?;
        std::fs::rename(&temporary, &path).map_err(|e| {
            let _ = std::fs::remove_file(&temporary);
            StoreError::Storage(e.to_string())
        })
    }
    pub fn verify(&self, oid: &str, size: u64) -> Result<bool, StoreError> {
        self.present(oid, size)
    }
    /// Streams an object into a temporary file, hashes it, then atomically publishes it.
    pub async fn write_stream(
        &self,
        oid: &str,
        reader: &mut (impl tokio::io::AsyncRead + Unpin),
    ) -> Result<(), StoreError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let path = self.path(oid)?;
        let parent = path
            .parent()
            .ok_or_else(|| StoreError::Storage("invalid storage path".into()))?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        let temporary = parent.join(format!(
            ".{oid}.{}.{}.tmp",
            std::process::id(),
            TEMPORARY_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = async {
            let mut file = tokio::fs::File::create(&temporary)
                .await
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let mut hash = sha2::Sha256::new();
            let mut total = 0_u64;
            let mut bytes = [0_u8; 8192];
            loop {
                let count = reader
                    .read(&mut bytes)
                    .await
                    .map_err(|e| StoreError::Storage(e.to_string()))?;
                if count == 0 {
                    break;
                }
                total += count as u64;
                if total > MAX_OBJECT_SIZE as u64 {
                    return Err(StoreError::Storage("object too large".into()));
                }
                hash.update(&bytes[..count]);
                file.write_all(&bytes[..count])
                    .await
                    .map_err(|e| StoreError::Storage(e.to_string()))?;
            }
            file.flush()
                .await
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            file.sync_all()
                .await
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            if format!("{:x}", hash.finalize()) != oid {
                return Err(StoreError::HashMismatch);
            }
            tokio::fs::rename(&temporary, &path)
                .await
                .map_err(|e| StoreError::Storage(e.to_string()))
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result
    }
    pub async fn open_stream(&self, oid: &str) -> Result<(u64, tokio::fs::File), StoreError> {
        let path = self.path(oid)?;
        let length = tokio::fs::metadata(&path)
            .await
            .map_err(|_| StoreError::NotFound)?
            .len();
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|_| StoreError::NotFound)?;
        Ok((length, file))
    }
    fn path(&self, oid: &str) -> Result<std::path::PathBuf, StoreError> {
        object_path(&self.objects, oid).ok_or(StoreError::InvalidOid)
    }
}

pub fn valid_oid(oid: &str) -> bool {
    oid.len() == 64
        && oid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn object_path(root: &std::path::Path, oid: &str) -> Option<std::path::PathBuf> {
    valid_oid(oid).then(|| root.join(&oid[..2]).join(&oid[2..4]).join(oid))
}
pub fn verify_bytes(oid: &str, bytes: &[u8]) -> bool {
    valid_oid(oid) && format!("{:x}", sha2::Sha256::digest(bytes)) == oid
}

#[cfg(test)]
mod tests {
    use sha2::Digest;
    #[test]
    fn validates_oid() {
        assert!(super::valid_oid(&"a".repeat(64)));
        assert!(!super::valid_oid("bad"));
    }
    #[tokio::test]
    async fn writes_streams_and_cleans_failed_uploads() {
        use tokio::io::AsyncReadExt;

        let root = std::env::temp_dir().join(format!("gilti-lfs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let repo = root.join("x.git");
        std::fs::create_dir_all(&repo).unwrap();
        let store = super::LfsStore::open(&root, "x").unwrap();
        let bytes = b"contents";
        let oid = format!("{:x}", sha2::Sha256::digest(bytes));

        let mut input = &bytes[..];
        store.write_stream(&oid, &mut input).await.unwrap();
        assert!(store.verify(&oid, bytes.len() as u64).unwrap());
        let (length, mut output) = store.open_stream(&oid).await.unwrap();
        let mut restored = Vec::new();
        output.read_to_end(&mut restored).await.unwrap();
        assert_eq!(length, bytes.len() as u64);
        assert_eq!(restored, bytes);

        let bad_oid = "a".repeat(64);
        let mut input = &bytes[..];
        assert_eq!(
            store.write_stream(&bad_oid, &mut input).await,
            Err(super::StoreError::HashMismatch)
        );
        let temporary_directory = repo.join("lfs/objects/aa/aa");
        assert!(
            std::fs::read_dir(temporary_directory)
                .unwrap()
                .next()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
