// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod about;
pub mod archive_signature;
pub mod blame;
pub mod object;
pub mod overview;
pub mod refs;
pub mod repositories;
pub mod repository;
pub mod revision;
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
