// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{DOCTYPE, html};

pub struct Query {
    search: Option<String>,
    sort: crate::models::repositories::Sort,
    sort_name: &'static str,
    offset: usize,
}

impl Query {
    pub fn from_request(query: &crate::RequestQuery) -> Self {
        let (sort, sort_name) = match query.value("GILTI_QUERY_SORT") {
            Some("desc") => (crate::models::repositories::Sort::Description, "desc"),
            Some("owner") => (crate::models::repositories::Sort::Owner, "owner"),
            Some("idle") => (crate::models::repositories::Sort::Idle, "idle"),
            _ => (crate::models::repositories::Sort::Name, "name"),
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
    let filter = crate::models::repositories::Filter {
        search: query.search.clone(),
        sort: query.sort,
        offset: query.offset,
        limit: 50,
    };
    let model = tokio::task::spawn_blocking(move || {
        crate::models::repositories::Repositories::load(std::path::Path::new(repositories), filter)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(crate::models::Error::Internal(error.to_string()));
        }
    };
    let document = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                title { (&*context.root_title) }
                meta name="generator" content="Gilti";
                meta name="robots" content="index, nofollow";
                link rel="stylesheet" type="text/css" href="/-/assets/cgit.css";
                script type="text/javascript" src="/-/assets/cgit.js" {}
                link rel="shortcut icon" href="/-/assets/favicon.ico";
            }
            body { div id="cgit" {
                table id="header" {
                    tr {
                        td class="logo" rowspan="2" { a href="/" { img src="/-/assets/cgit.png" alt="cgit logo"; } }
                        td class="main" { (&*context.root_title) }
                    }
                    tr { td class="sub" { (&*context.root_description) } }
                }
                table class="tabs" { tr {
                    td { a href="/" class="active" { "index" } }
                    td class="form" { form method="get" action="/" {
                        input type="search" name="q" size="10" value=(query.search.as_deref().unwrap_or_default());
                        input type="submit" value="search";
                    } }
                } }
                div class="content" { table summary="repository list" class="list nowrap" {
                    @if !model.rows.is_empty() {
                        tr class="nohover" {
                            th class="left" { a href=(sort_url("name", query.search.as_deref())) { "Name" } }
                            th class="left" { a href=(sort_url("desc", query.search.as_deref())) { "Description" } }
                            th class="left" { a href=(sort_url("owner", query.search.as_deref())) { "Owner" } }
                            th class="left" { a href=(sort_url("idle", query.search.as_deref())) { "Idle" } }
                            th class="left" { "Links" }
                        }
                    }
                    @for repository in &model.rows {
                        @let url = super::shared::repository_url(&repository.name);
                        tr {
                            td class="toplevel-repo" { a href=(&url) { (&repository.name) } }
                            td { a href=(&url) { (description(&repository.description)) } }
                            td { a href="/?q=" {} }
                            td { @if let Some(timestamp) = repository.timestamp { (super::shared::age(timestamp)) } }
                            td {
                                a class="button" href=(&url) { "summary" }
                                @if repository.populated {
                                    a class="button" href=(format!("{url}/+/HEAD/+/log")) { "log" }
                                    a class="button" href=(format!("{url}/+/HEAD/+/tree")) { "tree" }
                                }
                            }
                        }
                    }
                }
                @if model.offset > 0 || model.has_next {
                    ul class="pager" {
                        @if model.offset > 0 {
                            li { a href=(page_url(model.offset.saturating_sub(50), query.sort_name, query.search.as_deref())) { "[previous]" } }
                        }
                        @if model.has_next {
                            li { a href=(page_url(model.offset + 50, query.sort_name, query.search.as_deref())) { "[next]" } }
                        }
                    }
                }
                }
                div class="footer" { "generated by Gilti" }
            } }
        }
    }
    .into_string();
    let length = document.len();
    let body = if method == axum::http::Method::HEAD {
        axum::body::Body::empty()
    } else {
        axum::body::Body::from(document)
    };
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
        .header(axum::http::header::CONTENT_LENGTH, length)
        .body(body)
        .expect("repository list response is valid")
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
