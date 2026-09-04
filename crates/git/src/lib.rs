// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Framework-independent Git repository, LFS storage, and smart HTTP support.
//!
//! This crate deliberately exposes owned domain data only; `git2` handles never
//! cross its public API boundary.

pub mod about;
pub mod archive;
pub mod archive_signature;
pub mod backend;
pub mod blame;
pub mod commands;
pub mod commit;
pub mod diff;
pub mod history;
pub mod lfs;
pub mod object;
pub mod overview;
pub mod patch;
pub mod refs;
pub mod repositories;
pub mod repository;
pub mod revision;
pub mod stats;
pub mod tag;
pub mod time;
pub mod tree;

#[derive(Debug)]
pub enum Error {
    NotFound,
    Internal(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("not found"),
            Self::Internal(message) => formatter.write_str(message),
        }
    }
}
impl std::error::Error for Error {}
impl Error {
    pub(crate) fn from_git(error: git2::Error) -> Self {
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

/// A Git revision selector accepted by repository readers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Revision {
    Head,
    Ref(String),
    Commit(String),
}
