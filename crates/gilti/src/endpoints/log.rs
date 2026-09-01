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
    pub fn from_request(query: &crate::RequestQuery) -> Result<Self, ()> {
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
    let repositories = context.repositories;
    let name = route.repo;
    let revision = route.params.rev;
    let path = route.params.path;
    let options = gilti_git::history::Options {
        path: path.clone(),
        follow: query.follow,
        search: query.history(),
        offset: query.offset,
        limit: gilti_git::history::LOG_PAGE_SIZE,
        graph: true,
        ignore_whitespace: query.ignore_whitespace,
        include_statistics: true,
    };
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::history::History::load(
            std::path::Path::new(repositories),
            &name,
            revision,
            options,
        )
    })
    .await;
    let model = match model {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(gilti_git::Error::Internal(error.to_string())),
    };
    super::shared::render(
        context,
        &model.repository,
        &model.revision,
        super::shared::Page::Log,
        self::LogPage {
            model: &model,
            query: &query,
            path: path.as_deref(),
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
        assert!(super::Query::from_request(&crate::RequestQuery::default()).is_ok());
        let query = crate::request_query(Some("follow=2")).unwrap();
        assert!(super::Query::from_request(&query).is_err());
        let query = crate::request_query(Some("qt=range&q=-bad")).unwrap();
        assert!(super::Query::from_request(&query).is_err());
        let query = crate::request_query(Some("qt=range&q=HEAD..refs%2Fheads%2Fmain")).unwrap();
        assert!(super::Query::from_request(&query).is_ok());
    }
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::{
    components::{
        relative_time::RelativeTime,
        table::{DataTable, ListRow, RowStyle, TableFrame},
    },
    styles::classes::log,
};

/// Presentation page for a repository history listing.
struct LogPage<'a> {
    pub model: &'a gilti_git::history::History,
    pub query: &'a Query,
    pub path: Option<&'a str>,
}
impl Render for LogPage<'_> {
    fn render(&self) -> Markup {
        render_content(self.model, self.query, self.path)
    }
}

fn render_content(
    model: &gilti_git::history::History,
    query: &Query,
    path: Option<&str>,
) -> Markup {
    let base = format!(
        "{}/+/{}/+/log",
        crate::endpoints::shared::repository_url(&model.repository.name),
        crate::endpoints::shared::encode_path(&model.revision)
    );
    let base = path.map_or(base.clone(), |path| {
        format!("{base}/{}", crate::endpoints::shared::encode_path(path))
    });
    let columns = if model.graph { 6 } else { 5 };
    html! {
        form class=(log::RIGHT) method="get" action=(&base) {
            @if query.show_message { input type="hidden" name="showmsg" value="1"; }
            @if query.follow { input type="hidden" name="follow" value="1"; }
            @if query.ignore_whitespace { input type="hidden" name="ignorews" value="1"; }
            select name="qt" {
                option value="grep" selected[matches!(query.search, Search::Grep(_))] { "log msg" }
                option value="author" selected[matches!(query.search, Search::Author(_))] { "author" }
                option value="committer" selected[matches!(query.search, Search::Committer(_))] { "committer" }
                option value="range" selected[matches!(query.search, Search::Range(_))] { "range" }
            }
            input class=(log::TXT) type="search" size="10" name="q" value=(search_value(&query.search));
            input type="submit" value="search";
        }
        @if let Some(path) = path {
            div class=(log::PATH) {
                "path: " (path) " ("
                a href=(format!("{}{}", base, query.suffix(query.offset, query.show_message, !query.follow))) { (if query.follow { "unfollow" } else { "follow" }) }
                ")"
            }
        }
        (DataTable { summary: None, frame: TableFrame::List { nowrap: true }, content: html! {
            (ListRow { style: RowStyle::Static, content: html! {
                @if model.graph { th {} } @else { th class=(log::LEFT) { "Age" } }
                th class=(log::LEFT) { "Commit message (" a href=(format!("{}{}", base, query.suffix(query.offset, !query.show_message, query.follow))) { (if query.show_message { "Collapse" } else { "Expand" }) } ")" }
                th class=(log::LEFT) { "Author" }
                @if model.graph { th class=(log::LEFT) { "Age" } }
                th class=(log::LEFT) { "Files" }
                th class=(log::LEFT) { "Lines" }
            } })
            @for entry in &model.entries {
                @if model.graph {
                    @for continuation in &entry.graph_continuations {
                        (ListRow { style: RowStyle::Static, content: html! { td class=(log::COMMITGRAPH) { (graph(continuation)) } td colspan=(columns - 1) {} } })
                    }
                }
                (ListRow { style: if query.show_message { RowStyle::Highlighted } else { RowStyle::Normal }, content: html! {
                    @if model.graph { td class=(log::COMMITGRAPH) { (graph(&entry.graph)) } } @else { td { (age(&entry.committer)) } }
                    td class=[query.show_message.then_some(log::LOGSUBJECT)] {
                        a href=(format!("{}/+/{}", crate::endpoints::shared::repository_url(&model.repository.name), entry.oid)) { (&entry.subject) }
                        @for decoration in &entry.decorations {
                            @if let Some(reference) = &decoration.reference {
                                @if reference.starts_with("refs/tags/") {
                                    span class=(log::DECORATION) { " " a class=(log::TAG_DECO) href=(format!("{}/+/{}", crate::endpoints::shared::repository_url(&model.repository.name), crate::endpoints::shared::encode_path(reference))) { (&decoration.label) } }
                                } @else {
                                    @let link = entry.path.as_ref().map_or_else(|| format!("{}/+/{}/+/log{}", crate::endpoints::shared::repository_url(&model.repository.name), crate::endpoints::shared::encode_path(reference), query.suffix(0, query.show_message, query.follow)), |path| format!("{}/+/{}/+/log/{}{}", crate::endpoints::shared::repository_url(&model.repository.name), crate::endpoints::shared::encode_path(reference), crate::endpoints::shared::encode_path(path), query.suffix(0, query.show_message, query.follow)));
                                    span class=(log::DECORATION) { " " a class=(log::BRANCH_DECO) href=(link) { (&decoration.label) } }
                                }
                            } @else { span class=(log::DECORATION) { " " (&decoration.label) } }
                        }
                    }
                    td { (&entry.author.name) }
                    @if model.graph { td { (age(&entry.committer)) } }
                    td { (entry.files) }
                    td { span class=(log::DELETIONS) { "-" (entry.deletions) } "/" span class=(log::INSERTIONS) { "+" (entry.additions) } }
                } })
                @if query.show_message { (ListRow { style: RowStyle::PreserveStripeOnHover, content: html! { td colspan=(columns) class=(log::LOGMSG) { (&entry.body) @if let Some(notes) = &entry.notes { "\n" (notes) } } } }) }
            }
        } }.render())
        ul class=(log::PAGER) {
            @if query.offset > 0 { li { a href=(format!("{}{}", base, query.suffix(query.offset.saturating_sub(gilti_git::history::LOG_PAGE_SIZE), query.show_message, query.follow))) { "[prev]" } } }
            @if model.has_next { li { a href=(format!("{}{}", base, query.suffix(query.offset + gilti_git::history::LOG_PAGE_SIZE, query.show_message, query.follow))) { "[next]" } } }
        }
    }
}

fn graph(line: &str) -> Markup {
    html! {
        @for (index, character) in line.chars().enumerate() {
            span class=(graph_class(index / 2 % 6 + 1)) { (character) }
        }
    }
}

fn graph_class(column: usize) -> &'static str {
    [
        log::COLUMN1,
        log::COLUMN2,
        log::COLUMN3,
        log::COLUMN4,
        log::COLUMN5,
        log::COLUMN6,
    ][column - 1]
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

fn age(identity: &gilti_git::commit::Identity) -> Markup {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs().min(i64::MAX as u64) as i64);
    if now.saturating_sub(identity.timestamp) < 14 * 24 * 60 * 60 {
        return RelativeTime {
            timestamp: identity.timestamp,
        }
        .render();
    }
    let local = identity.timestamp + i64::from(identity.offset_minutes) * 60;
    let Some(value) = gilti_git::time::utc(local) else {
        return RelativeTime {
            timestamp: identity.timestamp,
        }
        .render();
    };
    html! {
        (format!(
            "{:04}-{:02}-{:02}",
            value.tm_year + 1900,
            value.tm_mon + 1,
            value.tm_mday
        ))
    }
}
