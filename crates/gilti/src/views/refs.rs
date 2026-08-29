// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, html};

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<()>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let model = tokio::task::spawn_blocking(move || {
        crate::models::refs::Refs::load(std::path::Path::new(repositories), &route.repo)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(crate::models::Error::Internal(error.to_string()));
        }
    };
    let content = content(&model);
    super::shared::render(
        context,
        &model.repository,
        "HEAD",
        super::shared::Page::Refs,
        content,
        &method,
    )
}

fn content(model: &crate::models::refs::Refs) -> Markup {
    let repo = super::shared::repository_url(&model.repository.name);
    html! { table class="list nowrap" {
        tr class="nohover" {
            th class="left" { "Branch" }
            th class="left" { "Commit message" }
            th class="left" { "Author" }
            th class="left" colspan="2" { "Age" }
        }
        @for branch in &model.branches {
            @let revision = super::shared::encode_path(&branch.reference);
            tr {
                td { a href=(format!("{repo}/+/{revision}/+/log")) { (&branch.name) } }
                td { a href=(format!("{repo}/+/{revision}")) { (&branch.subject) } }
                td { (&branch.author) }
                td colspan="2" { (super::shared::age(branch.timestamp)) }
            }
        }
        @if !model.tags.is_empty() {
            tr class="nohover" { td colspan="5" { " " } }
            tr class="nohover" {
                th class="left" { "Tag" }
                th class="left" { "Download" }
                th class="left" { "Author" }
                th class="left" colspan="2" { "Age" }
            }
        }
        @for tag in &model.tags {
            @let revision = super::shared::encode_path(&tag.reference);
            tr {
                td { a href=(format!("{repo}/+/{revision}")) { (&tag.name) } }
                td {
                    @if tag.downloadable {
                        @for (index, format) in ["tar", "tar.gz", "tar.bz2", "tar.lz", "tar.xz", "tar.zst", "zip"].iter().enumerate() {
                            @if index > 0 { "  " }
                            (archive_link(&repo, &revision, format))
                        }
                    } @else {
                        a href=(format!("{repo}/+/object/{}", tag.target)) { (&tag.target) }
                    }
                }
                td { (&tag.author) }
                td colspan="2" { (super::shared::age(tag.timestamp)) }
            }
        }
    } }
}

fn archive_link(repository: &str, revision: &str, format: &str) -> Markup {
    html! { a href=(format!("{repository}/+/{revision}/+/archive?format={format}")) { (format) } }
}
