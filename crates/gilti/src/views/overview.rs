// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, html};

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<()>,
    headers: &axum::http::HeaderMap,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let clone_url = clone_url(context, headers, &route.repo);
    let repositories = context.repositories;
    let model = tokio::task::spawn_blocking(move || {
        crate::models::overview::Overview::load(std::path::Path::new(repositories), &route.repo)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(crate::models::Error::Internal(error.to_string()));
        }
    };
    let content = content(&model, &clone_url);
    super::shared::render(
        context,
        &model.repository,
        "HEAD",
        super::shared::Page::Summary,
        content,
        &method,
    )
}

fn content(model: &crate::models::overview::Overview, clone_url: &str) -> Markup {
    let repo = super::shared::repository_url(&model.repository.name);
    html! {
        @if model.empty {
            div class="error" { "Repository seems to be empty" }
        }
        table summary="repository info" class="list nowrap" {
            @if !model.empty {
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
                }
                @if !model.commits.is_empty() {
                    tr class="nohover" { td colspan="5" { " " } }
                    tr class="nohover" {
                        th class="left" { "Age" }
                        th class="left" { "Commit message" }
                        th class="left" { "Author" }
                        th class="left" { "Files" }
                        th class="left" { "Lines" }
                    }
                    @for commit in &model.commits {
                        tr {
                            td { (super::shared::age(commit.timestamp)) }
                            td {
                                a href=(format!("{repo}/+/{}", commit.oid)) { (&commit.subject) }
                                @if !commit.decorations.is_empty() { span class="decoration" {
                                    @for decoration in &commit.decorations {
                                        @if let Some(reference) = &decoration.reference {
                                            @let revision = super::shared::encode_path(reference);
                                            a class=(if decoration.tag { "tag-annotated-deco" } else { "branch-deco" }) href=(if decoration.tag { format!("{repo}/+/{revision}") } else { format!("{repo}/+/{revision}/+/log") }) { (&decoration.label) }
                                        } @else {
                                            a class="deco" href=(format!("{repo}/+/{}", commit.oid)) { (&decoration.label) }
                                        }
                                    }
                                } }
                            }
                            td { (&commit.author) }
                            td { (commit.files) }
                            td { span class="deletions" { "-" (commit.deletions) } "/" span class="insertions" { "+" (commit.insertions) } }
                        }
                    }
                }
            }
            tr class="nohover" { td colspan="5" { " " } }
            tr class="nohover" { th class="left" colspan="5" { "Clone" } }
            tr { td colspan="5" { a rel="vcs-git" href=(clone_url) { (clone_url) } } }
        }
    }
}

fn archive_link(repository: &str, revision: &str, format: &str) -> Markup {
    html! { a href=(format!("{repository}/+/{revision}/+/archive?format={format}")) { (format) } }
}

fn clone_url(
    context: &super::shared::Context,
    headers: &axum::http::HeaderMap,
    repository: &str,
) -> String {
    let repository = super::shared::encode_path(repository);
    if !context.clone_prefix.is_empty() {
        return format!(
            "{}/{}.git",
            context.clone_prefix.trim_end_matches('/'),
            repository
        );
    }
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .filter(|value| matches!(*value, "http" | "https"))
        .unwrap_or("http");
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost");
    format!("{scheme}://{host}/{repository}.git")
}
