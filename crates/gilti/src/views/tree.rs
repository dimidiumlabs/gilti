// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{DOCTYPE, Markup, html};

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::RevisionPath>,
    format: Option<&str>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let model = tokio::task::spawn_blocking(move || {
        crate::models::tree::Tree::load(
            std::path::Path::new(repositories),
            &route.repo,
            route.params.rev,
            route.params.path,
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(crate::models::Error::Internal(error.to_string()));
        }
    };
    if format == Some("raw") {
        return raw(model, &method);
    }
    let content = html_content(&model);
    super::shared::render(
        context,
        &model.repository,
        &model.revision,
        super::shared::Page::Tree,
        content,
        &method,
    )
}

fn html_content(model: &crate::models::tree::Tree) -> Markup {
    let repo = super::shared::repository_url(&model.repository.name);
    let revision = super::shared::encode_path(&model.revision);
    let prefix = format!("{repo}/+/{revision}/+/tree");
    html! {
        @if let Some(path) = &model.path {
            div class="path" { "path: " (breadcrumbs(&prefix, path)) }
        }
        @match &model.content {
            crate::models::tree::Content::Directory { entries, .. } => {
                table summary="tree listing" class="list" {
                    tr class="nohover" {
                        th class="left" { "Mode" }
                        th class="left" { "Name" }
                        th class="right" { "Size" }
                        th {}
                    }
                    @for entry in entries {
                        @let path = super::shared::encode_path(&entry.path);
                        tr {
                            td class="ls-mode" { (filemode(entry.mode)) }
                            td {
                                @match entry.kind {
                                    crate::models::tree::Kind::Submodule => {
                                        span class="ls-mod" { (&entry.name) " @ " (&entry.oid) }
                                    }
                                    _ => {
                                        a href=(format!("{prefix}/{path}")) class=(entry_class(entry)) { (&entry.name) }
                                        @if let Some(target) = &entry.symlink_target {
                                            " -> " a href=(format!("{prefix}/{}", super::shared::encode_path(&symlink_path(&entry.path, target)))) class="ls-blob" { (target) }
                                        }
                                    }
                                }
                            }
                            td class="ls-size" { (entry.size) }
                            td {
                                a class="button" href=(format!("{repo}/+/{revision}/+/log/{path}")) { "log" }
                                a class="button" href=(format!("{repo}/+/stats")) { "stats" }
                                @if entry.kind != crate::models::tree::Kind::Submodule {
                                    a class="button" href=(format!("{prefix}/{path}?format=raw")) { "plain" }
                                }
                                @if entry.kind == crate::models::tree::Kind::Blob && entry.symlink_target.is_none() {
                                    a class="button" href=(format!("{repo}/+/{revision}/+/blame/{path}")) { "blame" }
                                }
                            }
                        }
                    }
                }
            }
            crate::models::tree::Content::Blob { oid, bytes, binary } => {
                "blob: " (oid) " ("
                a href=(format!("{prefix}/{}?format=raw", super::shared::encode_path(model.path.as_deref().unwrap_or_default()))) { "plain" }
                @if !binary { ") (" a href=(format!("{repo}/+/{revision}/+/blame/{}", super::shared::encode_path(model.path.as_deref().unwrap_or_default()))) { "blame" } }
                ")"
                @if *binary { (binary_blob(bytes)) } @else { (text_blob(bytes)) }
            }
        }
    }
}

fn filemode(mode: u32) -> String {
    let mut value = String::with_capacity(10);
    value.push(match mode {
        0o040000 => 'd',
        0o120000 => 'l',
        0o160000 => 'm',
        _ => '-',
    });
    for bit in [
        0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
    ] {
        value.push(if mode & bit == 0 {
            '-'
        } else {
            match bit {
                0o400 | 0o040 | 0o004 => 'r',
                0o200 | 0o020 | 0o002 => 'w',
                _ => 'x',
            }
        });
    }
    value
}

fn entry_class(entry: &crate::models::tree::Entry) -> String {
    if entry.kind == crate::models::tree::Kind::Tree {
        return "ls-dir".to_owned();
    }
    let extension = entry.name.rsplit_once('.').map(|(_, extension)| extension);
    extension.map_or_else(
        || "ls-blob".to_owned(),
        |extension| format!("ls-blob {extension}"),
    )
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
            a href=(format!("{prefix}/{}", super::shared::encode_path(&path))) { (part) }
        }
    }
}

fn text_blob(bytes: &[u8]) -> Markup {
    let text = String::from_utf8_lossy(bytes);
    let lines = if bytes.is_empty() {
        0
    } else {
        bytes.iter().filter(|byte| **byte == b'\n').count()
            + usize::from(bytes.last() != Some(&b'\n'))
    };
    html! { table summary="blob content" class="blob" { tr {
        td class="linenumbers" { pre {
            @for line in 1..=lines { a id=(format!("n{line}")) href=(format!("#n{line}")) { (line) } "\n" }
        } }
        td class="lines" { pre { code { (text) } } }
    } } }
}

fn binary_blob(bytes: &[u8]) -> Markup {
    html! { table summary="blob content" class="bin-blob" {
        tr { th { "ofs" } th { "hex dump" } th { "ascii" } }
        @for (row, chunk) in bytes.chunks(32).enumerate() {
            tr {
                td class="right" { (format!("{:04x}", row * 32)) }
                td class="hex" { (chunk.iter().map(|byte| format!("{byte:02x}")).collect::<Vec<_>>().join(" ")) }
                td class="hex" { (chunk.iter().map(|byte| if byte.is_ascii_graphic() { char::from(*byte) } else { '.' }).collect::<String>()) }
            }
        }
    } }
}

fn raw(model: crate::models::tree::Tree, method: &axum::http::Method) -> axum::response::Response {
    let filename = model.path.as_deref().map(content_disposition);
    match model.content {
        crate::models::tree::Content::Blob { oid, bytes, binary } => {
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
        crate::models::tree::Content::Directory { oid, entries } => {
            let repo = super::shared::repository_url(&model.repository.name);
            let revision = super::shared::encode_path(&model.revision);
            let current = model.path.as_deref().unwrap_or_default();
            let title = if current.is_empty() {
                "/".to_owned()
            } else {
                format!("/{current}/")
            };
            let document = html! {
                (DOCTYPE)
                html { head { title { (&title) } } body {
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
                                @if entry.kind == crate::models::tree::Kind::Submodule {
                                    (&entry.name) " @ " (&entry.oid)
                                } @else {
                                    a href=(format!("{repo}/+/{revision}/+/tree/{path}?format=raw")) { (&entry.name) @if entry.kind == crate::models::tree::Kind::Tree { "/" } }
                                }
                            }
                        }
                    }
                } }
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
