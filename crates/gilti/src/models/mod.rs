// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod about;
pub mod archive;
pub mod archive_signature;
pub mod blame;
pub mod commit;
pub mod diff;
pub mod object;
pub mod overview;
pub mod patch;
pub mod refs;
pub mod repositories;
pub mod repository;
pub mod revision;
pub mod stats;
pub mod tag;
pub mod tree;

#[derive(Debug)]
pub enum Error {
    NotFound,
    Internal(String),
}

impl Error {
    pub fn from_git(error: git2::Error) -> Self {
        if matches!(
            error.code(),
            git2::ErrorCode::NotFound | git2::ErrorCode::UnbornBranch
        ) {
            Self::NotFound
        } else {
            Self::Internal(error.message().to_owned())
        }
    }
}

#[cfg(test)]
mod migration_tests {
    #[test]
    fn loads_diff_commit_stats_archive_and_patch_models() {
        let root =
            std::env::temp_dir().join(format!("gilti-five-model-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let repository = git2::Repository::init_bare(root.join("example.git")).unwrap();
        let signature = git2::Signature::now("Model Tester", "model@example.test").unwrap();
        let first_blob = repository.blob(b"first\n").unwrap();
        let mut builder = repository.treebuilder(None).unwrap();
        builder.insert("file.txt", first_blob, 0o100644).unwrap();
        let first_tree_oid = builder.write().unwrap();
        drop(builder);
        let first_tree = repository.find_tree(first_tree_oid).unwrap();
        let first = repository
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "first",
                &first_tree,
                &[],
            )
            .unwrap();
        let first = repository.find_commit(first).unwrap();
        let first_oid = first.id();
        let second_blob = repository.blob(b"second\n").unwrap();
        let mut builder = repository.treebuilder(None).unwrap();
        builder.insert("file.txt", second_blob, 0o100644).unwrap();
        let second_tree_oid = builder.write().unwrap();
        drop(builder);
        let second_tree = repository.find_tree(second_tree_oid).unwrap();
        let second = repository
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "second\n\nbody",
                &second_tree,
                &[&first],
            )
            .unwrap();
        repository.set_head("refs/heads/main").unwrap();
        drop(first);
        drop(first_tree);
        drop(second_tree);
        drop(repository);

        let old = crate::router::Revision::Commit(first_oid.to_string());
        let new = crate::router::Revision::Commit(second.to_string());
        let options = super::diff::Options {
            context: 3,
            ignore_whitespace: false,
        };
        let diff = super::diff::Diff::load(
            &root,
            "example",
            Some(old.clone()),
            new.clone(),
            None,
            options,
        )
        .unwrap();
        assert_eq!(diff.files.len(), 1);
        assert_eq!((diff.additions, diff.deletions), (1, 1));
        let commit = super::commit::Commit::load(&root, "example", new.clone(), options).unwrap();
        assert_eq!(commit.subject, "second");
        assert_eq!(commit.parents, [first_oid.to_string()]);
        assert!(commit.diff.is_some());
        let stats =
            super::stats::Stats::load(&root, "example", super::stats::Period::Week).unwrap();
        assert_eq!(stats.totals.iter().sum::<usize>(), 2);
        let archive =
            super::archive::Archive::load(&root, "example", &new, Some("file.txt")).unwrap();
        assert_eq!(archive.oid, second.to_string());
        let patch = super::patch::Patch::load(&root, "example", &old, &new).unwrap();
        assert_eq!(patch.old_oid, first_oid.to_string());
        assert_eq!(patch.new_oid, second.to_string());
        std::fs::remove_dir_all(root).unwrap();
    }
}
