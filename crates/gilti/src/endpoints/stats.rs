// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub struct Query {
    pub period: gilti_git::stats::Period,
    pub code: &'static str,
    pub top: Option<usize>,
}

pub enum QueryError {
    BadRequest,
    NotFound,
}

impl Query {
    pub fn from_request(query: &crate::RequestQuery) -> Result<Self, QueryError> {
        let (period, code) = match query.value("GILTI_QUERY_PERIOD") {
            None | Some("w" | "week") => (gilti_git::stats::Period::Week, "w"),
            Some("m" | "month") => (gilti_git::stats::Period::Month, "m"),
            Some("q" | "quarter") => (gilti_git::stats::Period::Quarter, "q"),
            Some("y" | "year") => (gilti_git::stats::Period::Year, "y"),
            Some(_) => return Err(QueryError::NotFound),
        };
        let top = match query.value("GILTI_QUERY_OFFSET") {
            None | Some("0") => Some(10),
            Some("-1") => None,
            Some(value) => Some(
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or(QueryError::BadRequest)?,
            ),
        };
        Ok(Self { period, code, top })
    }
}

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<()>,
    query: Query,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let name = route.repo;
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::stats::Stats::load(std::path::Path::new(repositories), &name, query.period)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(gilti_git::Error::Internal(error.to_string())),
    };
    let content = Page {
        model: &model,
        query: &query,
    }
    .render();
    super::shared::render(
        context,
        &model.repository,
        "HEAD",
        super::shared::Page::Stats,
        content,
        &method,
    )
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

// Route-specific presentation for the repository page.

use maud::{Markup, Render, html};

use crate::styles::classes::stats;

struct Page<'a> {
    model: &'a gilti_git::stats::Stats,
    query: &'a crate::endpoints::stats::Query,
}
impl Render for Page<'_> {
    fn render(&self) -> Markup {
        content(self.model, self.query)
    }
}

pub fn content(model: &gilti_git::stats::Stats, query: &crate::endpoints::stats::Query) -> Markup {
    let shown = query
        .top
        .unwrap_or(model.authors.len())
        .min(model.authors.len());
    html! { div {
        div class=(stats::PANEL) {
            b { "stat options" }
            form method="get" { table {
                tr { td colspan="2" {} }
                tr { td class=(stats::LABEL) { "Period:" } td class=(stats::CTRL) {
                    select name="period" onchange="this.form.submit();" {
                        @for (code, name) in [("w","week"),("m","month"),("q","quarter"),("y","year")] {
                            option value=(code) selected[query.code == code] { (name) }
                        }
                    }
                } }
                tr { td class=(stats::LABEL) { "Authors:" } td class=(stats::CTRL) {
                    select name="ofs" onchange="this.form.submit();" {
                        @for value in [10_usize,25,50,100] {
                            option value=(value) selected[query.top == Some(value)] { (value) }
                        }
                        option value="-1" selected[query.top.is_none()] { "all" }
                    }
                } }
                tr { td {} td class=(stats::CTRL) { noscript { input type="submit" value="Reload"; } } }
            } }
        }
        h2 { "Commits per author per " (query.period.name()) }
        table class=(stats::TABLE) {
            tr { th { "Author" } @for label in &model.labels { th { (label) } } th { "Total" } }
            @for author in &model.authors[..shown] {
                tr { td class=(stats::LEFT) { (&author.name) }
                    @for count in &author.counts { td { (count) } }
                    td class=(stats::SUM) { (author.total) }
                }
            }
            @if shown < model.authors.len() {
                @let others = &model.authors[shown..];
                tr { td class=(stats::LEFT) { "Others (" (others.len()) ")" }
                    @for index in 0..4 { td { (others.iter().map(|author| author.counts[index]).sum::<usize>()) } }
                    td class=(stats::SUM) { (others.iter().map(|author| author.total).sum::<usize>()) }
                }
            }
            tr { td class=(stats::TOTAL) { "Total" }
                @for total in &model.totals { td class=(stats::SUM) { (total) } }
                td class=(stats::SUM) { (model.totals.iter().sum::<usize>()) }
            }
        }
    } }
}
