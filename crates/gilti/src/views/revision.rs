// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, html};

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::Revision>,
    query: super::diff::Query,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let name = route.repo.clone();
    let revision = route.params.clone();
    let model = tokio::task::spawn_blocking(move || {
        crate::models::commit::Commit::load(
            std::path::Path::new(repositories),
            &name,
            revision,
            query.options(),
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(crate::models::Error::Internal(error.to_string())),
    };
    let content = content(&model, query);
    super::shared::render_titled(
        context,
        &model.repository,
        &model.revision,
        super::shared::Page::Revision,
        Some(&model.subject),
        content,
        &method,
    )
}

fn content(model: &crate::models::commit::Commit, query: super::diff::Query) -> Markup {
    let repo = super::shared::repository_url(&model.repository.name);
    let revision = super::shared::encode_path(&model.revision);
    html! {
        (super::diff::content_controls(query))
        table summary="commit info" class="commit-info" {
            tr { th { "author" } td { (&model.author.name) " <" (&model.author.email) ">" }
                td class="right" { (timestamp(&model.author)) } }
            tr { th { "committer" } td { (&model.committer.name) " <" (&model.committer.email) ">" }
                td class="right" { (timestamp(&model.committer)) } }
            tr { th { "commit" } td colspan="2" class="oid" {
                a href=(format!("{repo}/+/{}", model.oid)) { (&model.oid) }
                @let patch_old = model.parents.first().map_or("HEAD", String::as_str);
                " (" a href=(format!("{repo}/+/patch/{patch_old}..{}", model.oid)) { "patch" } ")"
            } }
            tr { th { "tree" } td colspan="2" class="oid" {
                a href=(format!("{repo}/+/{revision}/+/tree")) { (&model.tree) }
            } }
            @for parent in &model.parents {
                tr { th { "parent" } td colspan="2" class="oid" {
                    a href=(format!("{repo}/+/{parent}")) { (parent) }
                    " (" a href=(format!("{repo}/+/diff/{parent}..{}", model.oid)) { "diff" } ")"
                } }
            }
            tr { th { "download" } td colspan="2" class="oid" {
                @for format in ["tar", "tar.gz", "tar.bz2", "tar.lz", "tar.xz", "tar.zst", "zip"] {
                    a href=(format!("{repo}/+/{revision}/+/archive?format={format}")) { (format) } br;
                }
            } }
        }
        div class="commit-subject" {
            (&model.subject)
            @for decoration in &model.decorations {
                " ("
                @if let Some(reference) = &decoration.reference {
                    @let reference = super::shared::encode_path(reference);
                    a href=(format!("{repo}/+/{reference}")) { (&decoration.label) }
                } @else { (&decoration.label) }
                ")"
            }
        }
        div class="commit-msg" { (&model.message) }
        @if let Some(notes) = &model.notes {
            div class="notes-header" { "Notes" }
            div class="notes" { (notes) }
            div class="notes-footer" {}
        }
        @if let Some(diff) = &model.diff {
            (super::diff::content(diff, query, None, false))
        }
    }
}

fn timestamp(identity: &crate::models::commit::Identity) -> String {
    let adjusted = identity.timestamp + i64::from(identity.offset_minutes) * 60;
    let mut value = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers are valid for the duration of the call.
    let value = unsafe {
        libc::gmtime_r(&adjusted, value.as_mut_ptr());
        value.assume_init()
    };
    let sign = if identity.offset_minutes < 0 {
        '-'
    } else {
        '+'
    };
    let offset = identity.offset_minutes.unsigned_abs();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {sign}{:02}{:02}",
        value.tm_year + 1900,
        value.tm_mon + 1,
        value.tm_mday,
        value.tm_hour,
        value.tm_min,
        value.tm_sec,
        offset / 60,
        offset % 60
    )
}
