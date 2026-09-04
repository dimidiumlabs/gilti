// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

#![allow(clippy::items_after_test_module)]

pub struct Query {
    pub offset: usize,
    pub show_message: bool,
    pub follow: bool,
    pub ignore_whitespace: bool,
    pub search: Search,
}

pub enum Search {
    None,
    Grep(String),
    Author(String),
    Committer(String),
    Range(String),
}

impl Query {
    pub fn from_request(query: &crate::daemon::RequestQuery) -> Result<Self, ()> {
        let offset = match query.value("GILTI_QUERY_OFFSET") {
            None => 0,
            Some(value) => value.parse().map_err(|_| ())?,
        };
        let boolean = |name| match query.value(name) {
            None | Some("0") => Ok(false),
            Some("1") => Ok(true),
            Some(_) => Err(()),
        };
        let search = match (
            query.value("GILTI_QUERY_GREP"),
            query.value("GILTI_QUERY_SEARCH"),
        ) {
            (None, None) => Search::None,
            (Some(_), None) | (None, Some(_)) => return Err(()),
            (Some("grep"), Some(value)) => Search::Grep(value.to_owned()),
            (Some("author"), Some(value)) => Search::Author(value.to_owned()),
            (Some("committer"), Some(value)) => Search::Committer(value.to_owned()),
            (Some("range"), Some(value)) if valid_range(value) => Search::Range(value.to_owned()),
            _ => return Err(()),
        };
        Ok(Self {
            offset,
            show_message: boolean("GILTI_QUERY_SHOWMSG")?,
            follow: boolean("GILTI_QUERY_FOLLOW")?,
            ignore_whitespace: boolean("GILTI_QUERY_IGNOREWS")?,
            search,
        })
    }

    fn history(&self) -> gilti_git::history::Search {
        match &self.search {
            Search::None => gilti_git::history::Search::None,
            Search::Grep(value) => gilti_git::history::Search::Grep(value.clone()),
            Search::Author(value) => gilti_git::history::Search::Author(value.clone()),
            Search::Committer(value) => gilti_git::history::Search::Committer(value.clone()),
            Search::Range(value) => gilti_git::history::Search::Range(value.clone()),
        }
    }

    pub(crate) fn suffix(&self, offset: usize, show_message: bool, follow: bool) -> String {
        let mut values = Vec::new();
        if offset != 0 {
            values.push(format!("ofs={offset}"));
        }
        if show_message {
            values.push("showmsg=1".to_owned());
        }
        if follow {
            values.push("follow=1".to_owned());
        }
        if self.ignore_whitespace {
            values.push("ignorews=1".to_owned());
        }
        match &self.search {
            Search::None => {}
            Search::Grep(value) => {
                values.push("qt=grep".to_owned());
                values.push(format!("q={}", query(value)));
            }
            Search::Author(value) => {
                values.push("qt=author".to_owned());
                values.push(format!("q={}", query(value)));
            }
            Search::Committer(value) => {
                values.push("qt=committer".to_owned());
                values.push(format!("q={}", query(value)));
            }
            Search::Range(value) => {
                values.push("qt=range".to_owned());
                values.push(format!("q={}", query(value)));
            }
        }
        if values.is_empty() {
            String::new()
        } else {
            format!("?{}", values.join("&"))
        }
    }
}

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::RevisionPath>,
    query: Query,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = std::sync::Arc::clone(&context.repositories);
    let name = route.repo;
    let revision = route.params.rev;
    let path = route.params.path;
    let options = gilti_git::history::Options {
        path: path.clone(),
        follow: query.follow,
        search: query.history(),
        offset: query.offset,
        limit: context.browser.log_commits_per_page,
        graph: true,
        ignore_whitespace: query.ignore_whitespace,
        include_statistics: true,
    };
    let git = std::sync::Arc::clone(&context.git);
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::history::History::load(&git, repositories.as_path(), &name, revision, options)
    })
    .await;
    let model = match model {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(gilti_git::Error::Internal(error.to_string())),
    };
    super::shared::render_with_options(
        context,
        &model.repository,
        &model.revision,
        super::shared::Page::Log,
        super::shared::RenderOptions {
            navigation_search: Some(render_navigation_search(&model, &query, path.as_deref())),
            ..Default::default()
        },
        self::LogPage {
            model: &model,
            query: &query,
            path: path.as_deref(),
            page_size: context.browser.log_commits_per_page,
        }
        .render(),
        &method,
    )
}

fn valid_range(value: &str) -> bool {
    let mut any = false;
    for selector in value.split_ascii_whitespace() {
        any = true;
        let valid = |selector: &str| {
            selector == "HEAD"
                || selector.starts_with("refs/")
                || (matches!(selector.len(), 40 | 64)
                    && selector.bytes().all(|byte| byte.is_ascii_hexdigit()))
        };
        if selector.starts_with('-') || !selector.split("..").all(valid) {
            return false;
        }
    }
    any
}

fn query(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn query_is_strict() {
        assert!(super::Query::from_request(&crate::daemon::RequestQuery::default()).is_ok());
        let query = crate::daemon::request_query(Some("follow=2")).unwrap();
        assert!(super::Query::from_request(&query).is_err());
        let query = crate::daemon::request_query(Some("qt=range&q=-bad")).unwrap();
        assert!(super::Query::from_request(&query).is_err());
        let query =
            crate::daemon::request_query(Some("qt=range&q=HEAD..refs%2Fheads%2Fmain")).unwrap();
        assert!(super::Query::from_request(&query).is_ok());
    }
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::{components::log_table::LogTable, styles::classes::log};

/// Presentation page for a repository history listing.
struct LogPage<'a> {
    pub model: &'a gilti_git::history::History,
    pub query: &'a Query,
    pub path: Option<&'a str>,
    pub page_size: usize,
}
impl Render for LogPage<'_> {
    fn render(&self) -> Markup {
        render_content(self.model, self.query, self.path, self.page_size)
    }
}

fn log_url(model: &gilti_git::history::History, path: Option<&str>) -> String {
    let base = format!(
        "{}/+/{}/+/log",
        crate::endpoints::shared::repository_url(&model.repository.name),
        crate::endpoints::shared::encode_path(&model.revision)
    );
    path.map_or(base.clone(), |path| {
        format!("{base}/{}", crate::endpoints::shared::encode_path(path))
    })
}

fn render_navigation_search(
    model: &gilti_git::history::History,
    query: &Query,
    path: Option<&str>,
) -> Markup {
    html! {
        form method="get" action=(log_url(model, path)) {
            @if query.show_message { input type="hidden" name="showmsg" value="1"; }
            @if query.follow { input type="hidden" name="follow" value="1"; }
            @if query.ignore_whitespace { input type="hidden" name="ignorews" value="1"; }
            select name="qt" {
                option value="grep" selected[matches!(query.search, Search::Grep(_))] { "log msg" }
                option value="author" selected[matches!(query.search, Search::Author(_))] { "author" }
                option value="committer" selected[matches!(query.search, Search::Committer(_))] { "committer" }
                option value="range" selected[matches!(query.search, Search::Range(_))] { "range" }
            }
            input type="search" size="10" name="q" value=(search_value(&query.search));
            button type="submit" { "search" };
        }
    }
}

fn render_content(
    model: &gilti_git::history::History,
    query: &Query,
    path: Option<&str>,
    page_size: usize,
) -> Markup {
    let base = log_url(model, path);
    let expand_url = format!(
        "{}{}",
        base,
        query.suffix(query.offset, !query.show_message, query.follow),
    );
    let branch_suffix = query.suffix(0, query.show_message, query.follow);
    html! {
        @if let Some(path) = path {
            div class=(log::PATH) {
                "path: " (path) " ("
                a href=(format!("{}{}", base, query.suffix(query.offset, query.show_message, !query.follow))) { (if query.follow { "unfollow" } else { "follow" }) }
                ")"
            }
        }
        (LogTable::History {
            model,
            show_message: query.show_message,
            expand_url,
            branch_suffix,
        })
        ul class=(log::PAGER) {
            @if query.offset > 0 { li { a href=(format!("{}{}", base, query.suffix(query.offset.saturating_sub(page_size), query.show_message, query.follow))) { "[prev]" } } }
            @if model.has_next { li { a href=(format!("{}{}", base, query.suffix(query.offset + page_size, query.show_message, query.follow))) { "[next]" } } }
        }
    }
}

fn search_value(search: &Search) -> &str {
    match search {
        Search::None => "",
        Search::Grep(value)
        | Search::Author(value)
        | Search::Committer(value)
        | Search::Range(value) => value,
    }
}
