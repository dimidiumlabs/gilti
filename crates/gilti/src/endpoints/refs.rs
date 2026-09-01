// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

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
        gilti_git::refs::Refs::load(std::path::Path::new(repositories), &route.repo)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(gilti_git::Error::Internal(error.to_string()));
        }
    };
    let content = RefsPage { model: &model }.render();
    super::shared::render(
        context,
        &model.repository,
        "HEAD",
        super::shared::Page::Refs,
        content,
        &method,
    )
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

// Repository refs presentation.

use maud::{Markup, Render, html};

use crate::components::{
    refs_table::{BranchesTable, TagsTable},
    table::TableGrid,
};

use crate::endpoints::shared;

struct RefsPage<'a> {
    pub model: &'a gilti_git::refs::Refs,
}

impl Render for RefsPage<'_> {
    fn render(&self) -> Markup {
        let repository_url = shared::repository_url(&self.model.repository.name);
        TableGrid {
            content: html! {
                @if !self.model.branches.is_empty() {
                    (BranchesTable {
                        repository_url: &repository_url,
                        branches: &self.model.branches,
                        nowrap: true,
                    })
                }
                @if !self.model.tags.is_empty() {
                    (TagsTable {
                        repository_url: &repository_url,
                        tags: &self.model.tags,
                        nowrap: true,
                    })
                }
            },
        }
        .render()
    }
}
