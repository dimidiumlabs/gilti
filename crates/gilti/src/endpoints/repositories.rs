// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::components::document;

pub struct Query {
    search: Option<String>,
    sort: gilti_git::repositories::Sort,
    sort_name: &'static str,
    offset: usize,
}

impl Query {
    pub fn from_request(query: &crate::RequestQuery) -> Self {
        let (sort, sort_name) = match query.value("GILTI_QUERY_SORT") {
            Some("desc") => (gilti_git::repositories::Sort::Description, "desc"),
            Some("owner") => (gilti_git::repositories::Sort::Owner, "owner"),
            Some("idle") => (gilti_git::repositories::Sort::Idle, "idle"),
            _ => (gilti_git::repositories::Sort::Name, "name"),
        };
        Self {
            search: query.value("GILTI_QUERY_SEARCH").map(str::to_owned),
            sort,
            sort_name,
            offset: query
                .value("GILTI_QUERY_OFFSET")
                .and_then(|offset| offset.parse().ok())
                .unwrap_or(0),
        }
    }
}

pub async fn serve(
    context: &super::shared::Context,
    query: Query,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let filter = gilti_git::repositories::Filter {
        search: query.search.clone(),
        sort: query.sort,
        offset: query.offset,
        limit: 50,
    };
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::repositories::Repositories::load(std::path::Path::new(repositories), filter)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(gilti_git::Error::Internal(error.to_string()));
        }
    };
    let page = RepositoriesPage {
        root_title: &context.root_title,
        description: &context.root_description,
        search: query.search.as_deref(),
        sort_name: query.sort_name,
        repositories: &model,
    };
    let document = document::render(&context.root_title, page.render()).into_string();
    let length = document.len();
    let body = axum::body::Body::from(document);
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
        .header(axum::http::header::CONTENT_LENGTH, length)
        .body(body)
        .expect("repository list response is valid")
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

// Global repository-list page and its list-specific presentation.

use maud::{Markup, Render, html};

use crate::{
    components::{
        layout::{Layout, NavigationLink},
        relative_time::RelativeTime,
        table::{DataTable, ListRow, RowStyle, TableFrame},
    },
    styles::classes::repositories,
};
use gilti_git::repositories::Repositories;

struct RepositoriesPage<'a> {
    pub root_title: &'a str,
    pub description: &'a str,
    pub search: Option<&'a str>,
    pub sort_name: &'a str,
    pub repositories: &'a Repositories,
}

impl Render for RepositoriesPage<'_> {
    fn render(&self) -> Markup {
        Layout {
            root_title: self.root_title,
            description: self.description,
            nested_links: Vec::new(),
            navigation_links: vec![NavigationLink {
                url: "/",
                label: "index",
                active: true,
            }],
            navigation_search: Some(html! {
                form method="get" action="/" {
                    input type="search" name="q" size="10" value=(self.search.unwrap_or_default());
                    button type="submit" { "search" };
                }
            }),
            sidebar: None,
            content: self.render_content(),
        }
        .render()
    }
}

impl RepositoriesPage<'_> {
    fn render_content(&self) -> Markup {
        html! {
            (DataTable {
                summary: Some("repository list"),
                frame: TableFrame::List { nowrap: true },
                content: html! {
                    @if !self.repositories.rows.is_empty() {
                        (ListRow { style: RowStyle::Static, content: html! {
                            th class=(repositories::LEFT) { a href=(sort_url("name", self.search)) { "Name" } }
                            th class=(repositories::LEFT) { a href=(sort_url("desc", self.search)) { "Description" } }
                            th class=(repositories::LEFT) { a href=(sort_url("owner", self.search)) { "Owner" } }
                            th class=(repositories::LEFT) { a href=(sort_url("idle", self.search)) { "Idle" } }
                            th class=(repositories::LEFT) { "Links" }
                        } })
                    }
                    @for repository in &self.repositories.rows {
                        @let url = repository_url(&repository.name);
                        tr {
                            td { a href=(&url) { (&repository.name) } }
                            td { a href=(&url) { (description(&repository.description)) } }
                            td { a href="/?q=" {} }
                            td { @if let Some(timestamp) = repository.timestamp { (RelativeTime { timestamp }) } }
                            td {
                                a href=(&url) { "summary" }
                                @if repository.populated {
                                    ", "
                                    a href=(format!("{url}/+/HEAD/+/log")) { "log" }
                                    ", "
                                    a href=(format!("{url}/+/HEAD/+/tree")) { "tree" }
                                }
                            }
                        }
                    }
                }
            }.render())
            @if self.repositories.offset > 0 || self.repositories.has_next {
                ul class=(repositories::PAGER) {
                    @if self.repositories.offset > 0 {
                        li { a href=(page_url(self.repositories.offset.saturating_sub(50), self.sort_name, self.search)) { "[previous]" } }
                    }
                    @if self.repositories.has_next {
                        li { a href=(page_url(self.repositories.offset + 50, self.sort_name, self.search)) { "[next]" } }
                    }
                }
            }
        }
    }
}

fn description(description: &str) -> String {
    let mut characters = description.chars();
    let shown = characters.by_ref().take(80).collect::<String>();
    if characters.next().is_some() {
        format!("{shown}...")
    } else {
        shown
    }
}

fn repository_url(repository: &str) -> String {
    format!("/{}", encode_path(repository))
}

fn encode_path(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'/' || byte == b'_' {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02x}").expect("writing to String cannot fail");
        }
    }
    encoded
}

fn sort_url(sort: &str, search: Option<&str>) -> String {
    search.map_or_else(
        || format!("/?s={sort}"),
        |search| format!("/?s={sort}&q={}", query_value(search)),
    )
}

fn page_url(offset: usize, sort: &str, search: Option<&str>) -> String {
    let mut url = format!("/?ofs={offset}&s={sort}");
    if let Some(search) = search {
        url.push_str("&q=");
        url.push_str(&query_value(search));
    }
    url
}

fn query_value(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}
