// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

#![allow(clippy::items_after_test_module)]

pub use crate::components::diff::Mode;

#[derive(Clone, Copy)]
pub struct Query {
    pub context: u32,
    pub ignore_whitespace: bool,
    pub mode: Mode,
}

impl Query {
    pub fn from_request(query: &crate::RequestQuery) -> Result<Self, ()> {
        let context = query
            .value("GILTI_QUERY_CONTEXT")
            .map(str::parse)
            .transpose()
            .map_err(|_| ())?
            .unwrap_or(3);
        let context = if context == 0 { 3 } else { context };
        if context > 40 {
            return Err(());
        }
        let ignore_whitespace = match query.value("GILTI_QUERY_IGNOREWS") {
            None | Some("0") => false,
            Some("1") => true,
            _ => return Err(()),
        };
        let mode = match query.value("GILTI_QUERY_DIFFTYPE") {
            None | Some("0") => Mode::Unified,
            Some("1") => Mode::SideBySide,
            Some("2") => Mode::StatOnly,
            _ => return Err(()),
        };
        Ok(Self {
            context,
            ignore_whitespace,
            mode,
        })
    }

    pub fn options(self) -> gilti_git::diff::Options {
        gilti_git::diff::Options {
            context: self.context,
            ignore_whitespace: self.ignore_whitespace,
        }
    }
}

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::Comparison>,
    query: Query,
    raw: bool,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let name = route.repo.clone();
    let old = route.params.old_rev.clone();
    let new = route.params.new_rev.clone();
    let path = route.params.path.clone();
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::diff::Diff::load(
            std::path::Path::new(repositories),
            &name,
            Some(old),
            new,
            path,
            query.options(),
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(gilti_git::Error::Internal(error.to_string())),
    };
    if raw {
        return raw_response(context, &route, &model, query, &method).await;
    }
    let revision = model.new_revision.clone();
    let content = crate::components::diff::Diff {
        model: &model,
        mode: query.mode,
        path: route.params.path.as_deref(),
    }
    .render();
    super::shared::render_with_options(
        context,
        &model.repository,
        &revision,
        super::shared::Page::Diff,
        super::shared::RenderOptions {
            sidebar: Some(options_sidebar(query)),
            ..Default::default()
        },
        content,
        &method,
    )
}

async fn raw_response(
    context: &super::shared::Context,
    route: &crate::router::RepoRoute<crate::router::Comparison>,
    model: &gilti_git::diff::Diff,
    query: Query,
    method: &axum::http::Method,
) -> axum::response::Response {
    let repository = match gilti_git::repository::path(
        std::path::Path::new(context.repositories),
        &route.repo,
    ) {
        Ok(repository) => repository,
        Err(error) => return super::error(error),
    };
    let output = match gilti_git::commands::raw_diff(
        &repository,
        model.old_oid.as_deref(),
        &model.new_oid,
        route.params.path.as_deref(),
        query.context,
        query.ignore_whitespace,
    )
    .await
    {
        Ok(output) => output,
        Err(error) => return super::error(error),
    };
    super::bytes_response("text/plain; charset=UTF-8", None, None, output, method)
}

#[cfg(test)]
mod tests {
    #[test]
    fn highlights_side_by_side_character_changes() {
        let common = crate::components::diff::common_subsequence("alpha", "aloha");
        assert_eq!(common, ['a', 'l', 'h', 'a']);
        assert_eq!(
            crate::components::diff::highlighted_segments("alpha", &common),
            [
                (false, "al".to_owned()),
                (true, "p".to_owned()),
                (false, "ha".to_owned()),
            ]
        );
    }

    #[test]
    fn options_sidebar_supplies_a_semantic_form_without_a_layout_wrapper() {
        let rendered = super::options_sidebar(super::Query {
            context: 3,
            ignore_whitespace: false,
            mode: super::Mode::Unified,
        })
        .into_string();

        assert!(rendered.starts_with("<form "));
        assert!(rendered.contains("<label for=\"diff-context\">"));
        assert!(rendered.contains("<button type=\"submit\">apply</button>"));
        assert!(!rendered.contains("onchange="));
        assert!(!rendered.contains("<noscript"));
        assert!(!rendered.contains("<table"));
        assert!(!rendered.contains("<aside"));
    }
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

pub fn options_sidebar(query: Query) -> Markup {
    html! {
        form method="get" aria-label="diff options" {
            strong { "diff options" }
            label for="diff-context" { "context:" }
            select id="diff-context" name="context" {
                @for value in [1_u32,2,3,4,5,6,7,8,9,10,15,20,25,30,35,40] {
                    option value=(value) selected[query.context == value] { (value) }
                }
            }
            label for="diff-whitespace" { "space:" }
            select id="diff-whitespace" name="ignorews" {
                option value="0" selected[!query.ignore_whitespace] { "include" }
                option value="1" selected[query.ignore_whitespace] { "ignore" }
            }
            label for="diff-mode" { "mode:" }
            select id="diff-mode" name="dt" {
                option value="0" selected[query.mode == Mode::Unified] { "unified" }
                option value="1" selected[query.mode == Mode::SideBySide] { "ssdiff" }
                option value="2" selected[query.mode == Mode::StatOnly] { "stat only" }
            }
            button type="submit" { "apply" }
        }
    }
}
