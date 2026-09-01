// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub fn selector(revision: &crate::Revision) -> String {
    match revision {
        crate::Revision::Head => "HEAD".to_owned(),
        crate::Revision::Ref(reference) | crate::Revision::Commit(reference) => reference.clone(),
    }
}

pub fn commit<'repo>(
    repository: &'repo git2::Repository,
    revision: &crate::Revision,
) -> Result<git2::Commit<'repo>, super::Error> {
    match revision {
        crate::Revision::Head => repository
            .head()
            .and_then(|head| head.peel_to_commit())
            .map_err(|_| super::Error::NotFound),
        crate::Revision::Ref(reference) => repository
            .find_reference(reference)
            .and_then(|reference| reference.peel_to_commit())
            .map_err(|_| super::Error::NotFound),
        crate::Revision::Commit(oid) => {
            let oid = git2::Oid::from_str_ext(oid, repository.object_format())
                .map_err(|_| super::Error::NotFound)?;
            repository
                .find_commit(oid)
                .map_err(|_| super::Error::NotFound)
        }
    }
}
