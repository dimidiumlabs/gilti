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
    let repositories = std::sync::Arc::clone(&context.repositories);
    let name = route.repo;
    let max_refs = context.browser.summary_refs;
    let max_commits = context.browser.summary_commits;
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::overview::Overview::load(repositories.as_path(), &name, max_refs, max_commits)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(gilti_git::Error::Internal(error.to_string()));
        }
    };
    let content = content(&model, &clone_url, &context.archive_formats);
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
        log_table::LogTable,
        refs_table::{BranchesTable, TagsTable},
        table::TableGrid,
    },
    styles::classes::overview,
};

pub fn content(
    model: &gilti_git::overview::Overview,
    clone_url: &str,
    archive_formats: &[gilti_git::archive::Format],
) -> Markup {
    let repository_url = crate::urls::repository(&model.repository.name);
    html! { div {
        @if model.empty {
            div class=(overview::ERROR) { "Repository seems to be empty" }
        }

        @if !model.tags.is_empty() || !model.branches.is_empty() || !model.commits.is_empty() {
            (TableGrid { content: html! {
                @if !model.tags.is_empty() {
                    (TagsTable {
                        repository_url: &repository_url,
                        tags: &model.tags,
                        archive_formats,
                        nowrap: false,
                    })
                }

                @if !model.branches.is_empty() {
                    (BranchesTable {
                        repository_url: &repository_url,
                        branches: &model.branches,
                        nowrap: false,
                    })
                }

                @if !model.commits.is_empty() {
                    (LogTable::Summary {
                        repository_url: &repository_url,
                        commits: &model.commits,
                    })
                }
            } })
        }

        section class=(overview::CLONE) aria-labelledby="clone-heading" {
            h2 id="clone-heading" class=(overview::CLONE_TITLE) { "Clone" }
            ul class=(overview::CLONE_LIST) {
                li { a rel="vcs-git" href=(clone_url) { (clone_url) } }
            }
        }
    } }
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

// Repository shell page. URLs and repository labels are presentation data supplied by views.

use maud::Render;

use crate::components::layout::{Layout, NavigationLink, NestedLink};

pub(crate) struct RepositoryPage<'a> {
    pub root_title: &'a str,
    pub repository_url: &'a str,
    pub repository_name: &'a str,
    pub description: &'a str,
    pub navigation_links: Vec<NavigationLink<'a>>,
    pub navigation_search: Option<Markup>,
    pub sidebar: Option<Markup>,
    pub content: Markup,
}

impl Render for RepositoryPage<'_> {
    fn render(&self) -> Markup {
        Layout {
            root_title: self.root_title,
            description: self.description,
            nested_links: vec![NestedLink {
                url: self.repository_url,
                label: self.repository_name,
            }],
            navigation_links: self.navigation_links.clone(),
            navigation_search: self.navigation_search.clone(),
            sidebar: self.sidebar.clone(),
            content: self.content.clone(),
        }
        .render()
    }
}
