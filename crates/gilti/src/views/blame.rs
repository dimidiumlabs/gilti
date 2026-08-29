// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, html};

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::RevisionFile>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let model = tokio::task::spawn_blocking(move || {
        crate::models::blame::Blame::load(
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
    let content = content(&model);
    super::shared::render(
        context,
        &model.repository,
        &model.revision,
        super::shared::Page::Tree,
        content,
        &method,
    )
}

fn content(model: &crate::models::blame::Blame) -> Markup {
    let repo = super::shared::repository_url(&model.repository.name);
    let revision = super::shared::encode_path(&model.revision);
    let path = super::shared::encode_path(&model.path);
    html! {
        div class="path" { "path: " (breadcrumbs(&repo, &revision, &model.path)) }
        "blob: " (&model.oid) " ("
        a href=(format!("{repo}/+/{revision}/+/tree/{path}?format=raw")) { "plain" }
        ") ("
        a href=(format!("{repo}/+/{revision}/+/tree/{path}")) { "tree" }
        ")"
        @if model.binary {
            div class="error" { "blob is binary." }
        } @else {
            (blame_table(model, &repo))
        }
    }
}

fn blame_table(model: &crate::models::blame::Blame, repo: &str) -> Markup {
    let lines = String::from_utf8_lossy(&model.bytes)
        .split_inclusive('\n')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    html! { table class="blame blob" {
        @for hunk in &model.hunks {
            @for offset in 0..hunk.lines {
                @let line = hunk.start + offset;
                tr class=[(offset % 2 == 0).then_some("alt")] {
                    @if offset == 0 {
                        td class="hashes" rowspan=(hunk.lines) {
                            span class="oid" title=(format!(
                                "author  {} <{}>  {}\ncommitter  {} <{}>\n\n{}",
                                hunk.author, hunk.author_email,
                                super::shared::age(hunk.timestamp),
                                hunk.committer, hunk.committer_email,
                                hunk.summary
                            )) {
                                a href=(format!("{repo}/+/{}", hunk.oid)) { (&hunk.short_oid) }
                            }
                            @if let Some(parent) = &hunk.parent {
                                " " a href=(format!(
                                    "{repo}/+/{parent}/+/blame/{}",
                                    super::shared::encode_path(&hunk.original_path)
                                )) title="Blame the previous revision" { "^" }
                            }
                        }
                    }
                    td class="linenumbers" { a id=(format!("n{line}")) href=(format!("#n{line}")) { (line) } }
                    td class="lines" { pre { code { (lines.get(line - 1).map_or("", String::as_str)) } } }
                }
            }
        }
    } }
}

fn breadcrumbs(repository: &str, revision: &str, path: &str) -> Markup {
    let parts = path.split('/').collect::<Vec<_>>();
    html! {
        a href=(format!("{repository}/+/{revision}/+/tree")) { "root" }
        @for (index, part) in parts.iter().enumerate() {
            "/"
            @let current = parts[..=index].join("/");
            @if index + 1 == parts.len() {
                a href=(format!("{repository}/+/{revision}/+/blame/{}", super::shared::encode_path(&current))) { (part) }
            } @else {
                a href=(format!("{repository}/+/{revision}/+/tree/{}", super::shared::encode_path(&current))) { (part) }
            }
        }
    }
}
