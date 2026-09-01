// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render};

use crate::{
    components::{document::Document, tabs::Tab},
    endpoints::overview::RepositoryPage,
};

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

pub fn render(
    context: &Context,
    repository: &gilti_git::repository::Info,
    revision: &str,
    active: Page,
    content: Markup,
    method: &axum::http::Method,
) -> axum::response::Response {
    render_titled(context, repository, revision, active, None, content, method)
}

pub fn render_titled(
    context: &Context,
    repository: &gilti_git::repository::Info,
    revision: &str,
    active: Page,
    page_title: Option<&str>,
    content: Markup,
    method: &axum::http::Method,
) -> axum::response::Response {
    let repo = repository_url(&repository.name);
    let rev = encode_path(revision);
    let title = page_title.map_or_else(
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
    let tree_url = format!("{repo}/+/{rev}/+/tree");
    let revision_url = format!("{repo}/+/{rev}");
    let diff_url = format!("{repo}/+/diff/HEAD..{rev}");
    let stats_url = format!("{repo}/+/stats");
    let mut tabs = Vec::new();
    if repository.has_readme {
        tabs.push(Tab {
            url: &about_url,
            label: "about",
            active: active == Page::About,
        });
    }
    tabs.extend([
        Tab {
            url: &repo,
            label: "summary",
            active: active == Page::Summary,
        },
        Tab {
            url: &refs_url,
            label: "refs",
            active: active == Page::Refs,
        },
        Tab {
            url: &log_url,
            label: "log",
            active: active == Page::Log,
        },
        Tab {
            url: &tree_url,
            label: "tree",
            active: active == Page::Tree,
        },
        Tab {
            url: &revision_url,
            label: "commit",
            active: active == Page::Revision,
        },
        Tab {
            url: &diff_url,
            label: "diff",
            active: active == Page::Diff,
        },
        Tab {
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
        tabs,
        content,
    };
    let document = Document {
        title: &title,
        body: page.render(),
    }
    .render()
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
        .expect("HTML response is valid")
}

pub fn repository_url(repository: &str) -> String {
    format!("/{}", encode_path(repository))
}

pub fn encode_path(value: &str) -> String {
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
