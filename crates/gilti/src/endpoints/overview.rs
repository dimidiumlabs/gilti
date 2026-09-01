// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

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
        gilti_git::overview::Overview::load(std::path::Path::new(repositories), &route.repo)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(gilti_git::Error::Internal(error.to_string()));
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
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

// Repository summary presentation.

use maud::{Markup, html};

use crate::{
    components::{
        relative_time::RelativeTime,
        table::{DataTable, ListRow, RowStyle, TableFrame},
    },
    styles::classes::overview,
};

pub fn content(model: &gilti_git::overview::Overview, clone_url: &str) -> Markup {
    let repo = crate::endpoints::shared::repository_url(&model.repository.name);
    html! { div {
        @if model.empty {
            div class=(overview::ERROR) { "Repository seems to be empty" }
        }
        (DataTable { summary: Some("repository info"), frame: TableFrame::List { nowrap: false }, content: html! {
            @if !model.empty {
                (ListRow { style: RowStyle::Static, content: html! {
                    th class=(overview::LEFT) { "Branch" }
                    th class=(overview::LEFT) { "Commit message" }
                    th class=(overview::LEFT) { "Author" }
                    th class=(overview::LEFT) colspan="2" { "Age" }
                } })
                @for branch in &model.branches {
                    @let revision = crate::endpoints::shared::encode_path(&branch.reference);
                    tr {
                        td { a href=(format!("{repo}/+/{revision}/+/log")) { (&branch.name) } }
                        td { a href=(format!("{repo}/+/{revision}")) { (&branch.subject) } }
                        td { (&branch.author) }
                        td colspan="2" { (RelativeTime { timestamp: branch.timestamp }) }
                    }
                }
                @if !model.tags.is_empty() {
                    (ListRow { style: RowStyle::Static, content: html! { td colspan="5" { " " } } })
                    (ListRow { style: RowStyle::Static, content: html! {
                        th class=(overview::LEFT) { "Tag" }
                        th class=(overview::LEFT) { "Download" }
                        th class=(overview::LEFT) { "Author" }
                        th class=(overview::LEFT) colspan="2" { "Age" }
                    } })
                    @for tag in &model.tags {
                        @let revision = crate::endpoints::shared::encode_path(&tag.reference);
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
                            td colspan="2" { (RelativeTime { timestamp: tag.timestamp }) }
                        }
                    }
                }
                @if !model.commits.is_empty() {
                    (ListRow { style: RowStyle::Static, content: html! { td colspan="5" { " " } } })
                    (ListRow { style: RowStyle::Static, content: html! {
                        th class=(overview::LEFT) { "Age" }
                        th class=(overview::LEFT) { "Commit message" }
                        th class=(overview::LEFT) { "Author" }
                        th class=(overview::LEFT) { "Files" }
                        th class=(overview::LEFT) { "Lines" }
                    } })
                    @for commit in &model.commits {
                        tr {
                            td { (RelativeTime { timestamp: commit.timestamp }) }
                            td {
                                a href=(format!("{repo}/+/{}", commit.oid)) { (&commit.subject) }
                                @if !commit.decorations.is_empty() { span class=(overview::DECORATION) {
                                    @for decoration in &commit.decorations {
                                        @if let Some(reference) = &decoration.reference {
                                            @let revision = crate::endpoints::shared::encode_path(reference);
                                            a class=(if decoration.tag { overview::TAG_ANNOTATED_DECO } else { overview::BRANCH_DECO }) href=(if decoration.tag { format!("{repo}/+/{revision}") } else { format!("{repo}/+/{revision}/+/log") }) { (&decoration.label) }
                                        } @else {
                                            a class=(overview::DECO) href=(format!("{repo}/+/{}", commit.oid)) { (&decoration.label) }
                                        }
                                    }
                                } }
                            }
                            td { (&commit.author) }
                            td { (commit.files) }
                            td { span class=(overview::DELETIONS) { "-" (commit.deletions) } "/" span class=(overview::INSERTIONS) { "+" (commit.insertions) } }
                        }
                    }
                }
            }
            (ListRow { style: RowStyle::Static, content: html! { td colspan="5" { " " } } })
            (ListRow { style: RowStyle::Static, content: html! { th class=(overview::LEFT) colspan="5" { "Clone" } } })
            tr { td colspan="5" { a rel="vcs-git" href=(clone_url) { (clone_url) } } }
        } }.render())
    } }
}

fn archive_link(repository: &str, revision: &str, format: &str) -> Markup {
    html! { a href=(format!("{repository}/+/{revision}/+/archive?format={format}")) { (format) } }
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

// Repository shell page. URLs and repository labels are presentation data supplied by views.

use maud::Render;

use crate::components::{
    header::{Header, LinkLabel},
    layout::ContentLayout,
    tabs::{Tab, Tabs},
};

pub(crate) struct RepositoryPage<'a> {
    pub root_title: &'a str,
    pub repository_url: &'a str,
    pub repository_name: &'a str,
    pub description: &'a str,
    pub tabs: Vec<Tab<'a>>,
    pub content: Markup,
}

impl Render for RepositoryPage<'_> {
    fn render(&self) -> Markup {
        html! { div {
            (Header { home_url: "/", logo_url: "/-/assets/gilti.png", root_title: self.root_title,
                repository: Some(LinkLabel { url: self.repository_url, label: self.repository_name }), description: self.description })
            (Tabs { items: self.tabs.clone(), trailing: None })
            (ContentLayout { content: self.content.clone(), footer: html! { "generated by Gilti" } })
        } }
    }
}
