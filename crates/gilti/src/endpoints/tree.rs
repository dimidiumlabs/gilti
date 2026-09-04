// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Render, html};

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::RevisionPath>,
    format: Option<&str>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = std::sync::Arc::clone(&context.repositories);
    let binary_detection_bytes = usize::try_from(context.browser.binary_detection_bytes.as_u64())
        .expect("binary detection limit fits usize");
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::tree::Tree::load(
            repositories.as_path(),
            &route.repo,
            route.params.rev,
            route.params.path,
            binary_detection_bytes,
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(gilti_git::Error::Internal(error.to_string()));
        }
    };
    if format == Some("raw") {
        return raw(model, &method);
    }
    let content = self::TreePage { model: &model }.render();
    super::shared::render(
        context,
        &model.repository,
        &model.revision,
        super::shared::Page::Tree,
        content,
        &method,
    )
}

fn raw(model: gilti_git::tree::Tree, method: &axum::http::Method) -> axum::response::Response {
    let filename = model.path.as_deref().map(content_disposition);
    match model.content {
        gilti_git::tree::Content::Blob { oid, bytes, binary } => {
            let length = bytes.len();
            let body = if method == axum::http::Method::HEAD {
                axum::body::Body::empty()
            } else {
                axum::body::Body::from(bytes)
            };
            axum::response::Response::builder()
                .status(axum::http::StatusCode::OK)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    if binary {
                        "application/octet-stream"
                    } else {
                        "text/plain; charset=UTF-8"
                    },
                )
                .header(axum::http::header::CONTENT_LENGTH, length)
                .header(axum::http::header::ETAG, format!("\"{oid}\""))
                .header(
                    axum::http::header::CONTENT_DISPOSITION,
                    filename.as_deref().unwrap_or("inline"),
                )
                .body(body)
                .expect("raw blob response is valid")
        }
        gilti_git::tree::Content::Directory { oid, entries } => {
            let repo = super::shared::repository_url(&model.repository.name);
            let revision = super::shared::encode_path(&model.revision);
            let current = model.path.as_deref().unwrap_or_default();
            let title = if current.is_empty() {
                "/".to_owned()
            } else {
                format!("/{current}/")
            };
            let content = html! {
                h2 { (&title) }
                ul {
                    @if let Some((parent, _)) = current.rsplit_once('/') {
                        li { a href=(format!("{repo}/+/{revision}/+/tree/{}?format=raw", super::shared::encode_path(parent))) { "../" } }
                    } @else if !current.is_empty() {
                        li { a href=(format!("{repo}/+/{revision}/+/tree?format=raw")) { "../" } }
                    }
                    @for entry in entries {
                        @let path = super::shared::encode_path(&entry.path);
                        li {
                            @if entry.kind == gilti_git::tree::Kind::Submodule {
                                (&entry.name) " @ " (&entry.oid)
                            } @else {
                                a href=(format!("{repo}/+/{revision}/+/tree/{path}?format=raw")) { (&entry.name) @if entry.kind == gilti_git::tree::Kind::Tree { "/" } }
                            }
                        }
                    }
                }
            };
            let document = crate::components::document::render(&title, content).into_string();
            let length = document.len();
            let body = axum::body::Body::from(document);
            axum::response::Response::builder()
                .status(axum::http::StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
                .header(axum::http::header::CONTENT_LENGTH, length)
                .header(axum::http::header::ETAG, format!("\"{oid}\""))
                .body(body)
                .expect("raw directory response is valid")
        }
    }
}

fn content_disposition(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let fallback = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!(
        "inline; filename=\"{fallback}\"; filename*=UTF-8''{}",
        super::shared::encode_path(name)
    )
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::Markup;

use crate::{
    components::{
        code_block::{CodeBlock, LineNumbers, text_lines},
        table::{DataTable, ListRow, RowStyle, TableFrame},
    },
    styles::classes::tree,
};

/// Presentation page for a tree directory or blob.
struct TreePage<'a> {
    pub model: &'a gilti_git::tree::Tree,
}
impl Render for TreePage<'_> {
    fn render(&self) -> Markup {
        render_content(self.model)
    }
}

fn render_content(model: &gilti_git::tree::Tree) -> Markup {
    let repo = crate::endpoints::shared::repository_url(&model.repository.name);
    let revision = crate::endpoints::shared::encode_path(&model.revision);
    let prefix = format!("{repo}/+/{revision}/+/tree");
    html! {
        @if let Some(path) = &model.path {
            div class=(tree::PATH) { "path: " (breadcrumbs(&prefix, path)) }
        }
        @match &model.content {
            gilti_git::tree::Content::Directory { entries, .. } => {
                (DataTable { summary: Some("tree listing"), frame: TableFrame::List { nowrap: false }, content: html! {
                    (ListRow { style: RowStyle::Static, content: html! {
                        th class=(tree::LEFT) { "Mode" }
                        th class=(tree::LEFT) { "Name" }
                        th class=(tree::RIGHT) { "Size" }
                        th {}
                    } })
                    @for entry in entries {
                        @let path = crate::endpoints::shared::encode_path(&entry.path);
                        tr {
                            td class=(tree::LS_MODE) { (crate::components::file_mode(entry.mode)) }
                            td {
                                @match entry.kind {
                                    gilti_git::tree::Kind::Submodule => {
                                        span class=(tree::LS_MOD) { (&entry.name) " @ " (&entry.oid) }
                                    }
                                    _ => {
                                        a href=(format!("{prefix}/{path}")) class=(entry_class(entry)) { (&entry.name) }
                                        @if let Some(target) = &entry.symlink_target {
                                            " -> " a href=(format!("{prefix}/{}", crate::endpoints::shared::encode_path(&symlink_path(&entry.path, target)))) class=(tree::LS_BLOB) { (target) }
                                        }
                                    }
                                }
                            }
                            td class=(tree::LS_SIZE) { (entry.size) }
                            td {
                                a href=(format!("{repo}/+/{revision}/+/log/{path}")) { "log" }
                                ", "
                                a href=(format!("{repo}/+/stats")) { "stats" }
                                @if entry.kind != gilti_git::tree::Kind::Submodule {
                                    ", "
                                    a href=(format!("{prefix}/{path}?format=raw")) { "plain" }
                                }
                                @if entry.kind == gilti_git::tree::Kind::Blob && entry.symlink_target.is_none() {
                                    ", "
                                    a href=(format!("{repo}/+/{revision}/+/blame/{path}")) { "blame" }
                                }
                            }
                        }
                    }
                } }.render())
            }
            gilti_git::tree::Content::Blob { oid, bytes, binary } => {
                "blob: " (oid) " ("
                a href=(format!("{prefix}/{}?format=raw", crate::endpoints::shared::encode_path(model.path.as_deref().unwrap_or_default()))) { "plain" }
                @if !binary { ") (" a href=(format!("{repo}/+/{revision}/+/blame/{}", crate::endpoints::shared::encode_path(model.path.as_deref().unwrap_or_default()))) { "blame" } }
                ")"
                @if *binary { (binary_blob(bytes)) } @else { (text_blob(bytes)) }
            }
        }
    }
}

fn entry_class(entry: &gilti_git::tree::Entry) -> String {
    if entry.kind == gilti_git::tree::Kind::Tree {
        return tree::LS_DIR.to_owned();
    }
    let extension = entry.name.rsplit_once('.').map(|(_, extension)| extension);
    extension.map_or_else(|| tree::LS_BLOB.to_owned(), |_| tree::LS_BLOB.to_owned())
}

fn symlink_path(path: &str, target: &str) -> String {
    let mut parts = path.rsplit_once('/').map_or_else(Vec::new, |(parent, _)| {
        parent.split('/').collect::<Vec<_>>()
    });
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

fn breadcrumbs(prefix: &str, path: &str) -> Markup {
    let parts = path.split('/').collect::<Vec<_>>();
    html! {
        a href=(prefix) { "root" }
        @for (index, part) in parts.iter().enumerate() {
            "/"
            @let path = parts[..=index].join("/");
            a href=(format!("{prefix}/{}", crate::endpoints::shared::encode_path(&path))) { (part) }
        }
    }
}

fn text_blob(bytes: &[u8]) -> Markup {
    let text = String::from_utf8_lossy(bytes);
    CodeBlock {
        summary: "blob content",
        numbers: LineNumbers::Single,
        annotations: false,
        lines: text_lines(&text),
    }
    .render()
}

fn binary_blob(bytes: &[u8]) -> Markup {
    html! { table summary="blob content" class=(tree::BIN_BLOB) {
        tr { th { "ofs" } th { "hex dump" } th { "ascii" } }
        @for (row, chunk) in bytes.chunks(32).enumerate() {
            tr {
                td class=(tree::RIGHT) { (format!("{:04x}", row * 32)) }
                td class=(tree::HEX) { (chunk.iter().map(|byte| format!("{byte:02x}")).collect::<Vec<_>>().join(" ")) }
                td class=(tree::HEX) { (chunk.iter().map(|byte| if byte.is_ascii_graphic() { char::from(*byte) } else { '.' }).collect::<String>()) }
            }
        }
    } }
}
