// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render};

use crate::{
    components::{document, layout::NavigationLink},
    endpoints::overview::RepositoryPage,
};

pub use crate::urls::{encode_path, repository as repository_url};

#[derive(Clone)]
pub struct Context {
    pub repositories: &'static str,
    pub root_title: std::sync::Arc<str>,
    pub root_description: std::sync::Arc<str>,
    pub clone_prefix: std::sync::Arc<str>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Page {
    About,
    Summary,
    Refs,
    Log,
    Tree,
    Revision,
    Diff,
    Stats,
}

#[derive(Default)]
pub struct RenderOptions<'a> {
    pub page_title: Option<&'a str>,
    pub navigation_search: Option<Markup>,
    pub sidebar: Option<Markup>,
}

pub fn render(
    context: &Context,
    repository: &gilti_git::repository::Info,
    revision: &str,
    active: Page,
    content: Markup,
    method: &axum::http::Method,
) -> axum::response::Response {
    render_with_options(
        context,
        repository,
        revision,
        active,
        RenderOptions::default(),
        content,
        method,
    )
}

pub fn render_with_options(
    context: &Context,
    repository: &gilti_git::repository::Info,
    revision: &str,
    active: Page,
    options: RenderOptions<'_>,
    content: Markup,
    _method: &axum::http::Method,
) -> axum::response::Response {
    let repo = repository_url(&repository.name);
    let rev = encode_path(revision);
    let title = options.page_title.map_or_else(
        || format!("{} - {}", repository.name, repository.description),
        |page_title| {
            format!(
                "{page_title} - {} - {}",
                repository.name, repository.description
            )
        },
    );
    let about_url = format!("{repo}/+/about");
    let refs_url = format!("{repo}/+/refs");
    let log_url = format!("{repo}/+/{rev}/+/log");
    let revision_url = format!("{repo}/+/{rev}");
    let tree_url = format!("{repo}/+/{rev}/+/tree");
    let diff_url = format!("{repo}/+/diff/HEAD..{rev}");
    let stats_url = format!("{repo}/+/stats");
    let mut navigation_links = Vec::new();
    if repository.has_readme {
        navigation_links.push(NavigationLink {
            url: &about_url,
            label: "about",
            active: active == Page::About,
        });
    }
    navigation_links.extend([
        NavigationLink {
            url: &repo,
            label: "summary",
            active: active == Page::Summary,
        },
        NavigationLink {
            url: &refs_url,
            label: "refs",
            active: active == Page::Refs,
        },
        NavigationLink {
            url: &log_url,
            label: "log",
            active: active == Page::Log,
        },
        NavigationLink {
            url: &tree_url,
            label: "tree",
            active: active == Page::Tree,
        },
        NavigationLink {
            url: &revision_url,
            label: "commit",
            active: active == Page::Revision,
        },
        NavigationLink {
            url: &diff_url,
            label: "diff",
            active: active == Page::Diff,
        },
        NavigationLink {
            url: &stats_url,
            label: "stats",
            active: active == Page::Stats,
        },
    ]);
    let page = RepositoryPage {
        root_title: &context.root_title,
        repository_url: &repo,
        repository_name: &repository.name,
        description: &repository.description,
        navigation_links,
        navigation_search: options.navigation_search,
        sidebar: options.sidebar,
        content,
    };
    let document = document::render(&title, page.render()).into_string();
    let length = document.len();
    let body = axum::body::Body::from(document);
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
        .header(axum::http::header::CONTENT_LENGTH, length)
        .body(body)
        .expect("HTML response is valid")
}
