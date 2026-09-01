// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

pub struct Tag {
    pub repository: super::repository::Info,
    pub reference: String,
    pub name: String,
    pub oid: String,
    pub targets: Vec<Target>,
    pub annotated: bool,
    pub tagger: String,
    pub tagger_email: String,
    pub timestamp: Option<i64>,
    pub message: String,
    pub downloadable: bool,
}

pub struct Target {
    pub oid: String,
    pub commit: bool,
}

impl Tag {
    pub fn load(root: &Path, name: &str, reference: String) -> Result<Self, super::Error> {
        if !reference.starts_with("refs/tags/") {
            return Err(super::Error::NotFound);
        }
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let git_reference = repository
            .find_reference(&reference)
            .map_err(super::Error::from_git)?;
        let oid = git_reference
            .resolve()
            .map_err(super::Error::from_git)?
            .target()
            .ok_or(super::Error::NotFound)?;
        let object = repository
            .find_object(oid, None)
            .map_err(super::Error::from_git)?;
        let downloadable = object.peel_to_commit().is_ok();
        let (targets, annotated, tagger, tagger_email, timestamp, message) =
            if let Some(tag) = object.as_tag() {
                let tagger = tag.tagger();
                (
                    targets(tag)?,
                    true,
                    tagger.as_ref().map_or_else(String::new, signature_name),
                    tagger.as_ref().map_or_else(String::new, signature_email),
                    tagger.as_ref().map(|tagger| tagger.when().seconds()),
                    String::from_utf8_lossy(tag.message_bytes().unwrap_or_default()).into_owned(),
                )
            } else {
                (
                    vec![Target {
                        oid: object.id().to_string(),
                        commit: object.kind() == Some(git2::ObjectType::Commit),
                    }],
                    false,
                    String::new(),
                    String::new(),
                    None,
                    String::new(),
                )
            };
        Ok(Self {
            repository: info,
            reference: reference.clone(),
            name: reference["refs/tags/".len()..].to_owned(),
            oid: oid.to_string(),
            targets,
            annotated,
            tagger,
            tagger_email,
            timestamp,
            message,
            downloadable,
        })
    }
}

fn targets(tag: &git2::Tag<'_>) -> Result<Vec<Target>, super::Error> {
    let mut targets = Vec::new();
    let mut object = tag.target().map_err(super::Error::from_git)?;
    loop {
        targets.push(Target {
            oid: object.id().to_string(),
            commit: object.kind() == Some(git2::ObjectType::Commit),
        });
        let Some(tag) = object.as_tag() else {
            break;
        };
        object = tag.target().map_err(super::Error::from_git)?;
    }
    Ok(targets)
}

fn signature_name(signature: &git2::Signature<'_>) -> String {
    signature
        .name()
        .map(str::to_owned)
        .unwrap_or_else(|_| String::from_utf8_lossy(signature.name_bytes()).into_owned())
}

fn signature_email(signature: &git2::Signature<'_>) -> String {
    signature
        .email()
        .map(str::to_owned)
        .unwrap_or_else(|_| String::from_utf8_lossy(signature.email_bytes()).into_owned())
}
