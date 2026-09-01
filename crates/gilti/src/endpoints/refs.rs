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

use crate::{
    components::{
        relative_time::RelativeTime,
        table::{DataTable, ListRow, RowStyle, TableFrame},
    },
    styles::classes::refs,
};

use crate::endpoints::shared;

struct RefsPage<'a> {
    pub model: &'a gilti_git::refs::Refs,
}

impl Render for RefsPage<'_> {
    fn render(&self) -> Markup {
        let repo = shared::repository_url(&self.model.repository.name);
        html! { div { (DataTable { summary: None, frame: TableFrame::List { nowrap: true }, content: html! {
                (ListRow { style: RowStyle::Static, content: html! { th class=(refs::LEFT) { "Branch" } th class=(refs::LEFT) { "Commit message" } th class=(refs::LEFT) { "Author" } th class=(refs::LEFT) colspan="2" { "Age" } } })
                @for branch in &self.model.branches {
                    @let revision = shared::encode_path(&branch.reference);
                    tr { td { a href=(format!("{repo}/+/{revision}/+/log")) { (&branch.name) } } td { a href=(format!("{repo}/+/{revision}")) { (&branch.subject) } } td { (&branch.author) } td colspan="2" { (RelativeTime { timestamp: branch.timestamp }) } }
                }
                @if !self.model.tags.is_empty() { (ListRow { style: RowStyle::Static, content: html! { td colspan="5" { " " } } }) (ListRow { style: RowStyle::Static, content: html! { th class=(refs::LEFT) { "Tag" } th class=(refs::LEFT) { "Download" } th class=(refs::LEFT) { "Author" } th class=(refs::LEFT) colspan="2" { "Age" } } }) }
                @for tag in &self.model.tags {
                    @let revision = shared::encode_path(&tag.reference);
                    tr { td { a href=(format!("{repo}/+/{revision}")) { (&tag.name) } } td { @if tag.downloadable { @for (index, format) in ["tar", "tar.gz", "tar.bz2", "tar.lz", "tar.xz", "tar.zst", "zip"].iter().enumerate() { @if index > 0 { "  " } a href=(format!("{repo}/+/{revision}/+/archive?format={format}")) { (format) } } } @else { a href=(format!("{repo}/+/object/{}", tag.target)) { (&tag.target) } } } td { (&tag.author) } td colspan="2" { (RelativeTime { timestamp: tag.timestamp }) } }
                }
            } }) }
        }
    }
}
