// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

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
        gilti_git::blame::Blame::load(
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
            return super::error(gilti_git::Error::Internal(error.to_string()));
        }
    };
    let content = self::BlamePage { model: &model }.render();
    super::shared::render(
        context,
        &model.repository,
        &model.revision,
        super::shared::Page::Tree,
        content,
        &method,
    )
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::{components::relative_time::RelativeTime, styles::classes::blame};

/// Presentation page for annotated source lines.
struct BlamePage<'a> {
    pub model: &'a gilti_git::blame::Blame,
}
impl Render for BlamePage<'_> {
    fn render(&self) -> Markup {
        render_content(self.model)
    }
}

fn render_content(model: &gilti_git::blame::Blame) -> Markup {
    let repo = crate::endpoints::shared::repository_url(&model.repository.name);
    let revision = crate::endpoints::shared::encode_path(&model.revision);
    let path = crate::endpoints::shared::encode_path(&model.path);
    html! {
        div class=(blame::PATH) { "path: " (breadcrumbs(&repo, &revision, &model.path)) }
        "blob: " (&model.oid) " ("
        a href=(format!("{repo}/+/{revision}/+/tree/{path}?format=raw")) { "plain" }
        ") ("
        a href=(format!("{repo}/+/{revision}/+/tree/{path}")) { "tree" }
        ")"
        @if model.binary {
            div class=(blame::ERROR) { "blob is binary." }
        } @else {
            (blame_table(model, &repo))
        }
    }
}

fn blame_table(model: &gilti_git::blame::Blame, repo: &str) -> Markup {
    let lines = String::from_utf8_lossy(&model.bytes)
        .split_inclusive('\n')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    html! { table class=(blame::BLAME) {
        @for hunk in &model.hunks {
            @for offset in 0..hunk.lines {
                @let line = hunk.start + offset;
                tr class=[(offset % 2 == 0).then_some(blame::ALT)] {
                    @if offset == 0 {
                        td class=(blame::HASHES) rowspan=(hunk.lines) {
                            span class=(blame::OID) title=(format!(
                                "author  {} <{}>  {}\ncommitter  {} <{}>\n\n{}",
                                hunk.author, hunk.author_email,
                                RelativeTime::label(hunk.timestamp),
                                hunk.committer, hunk.committer_email,
                                hunk.summary
                            )) {
                                a href=(format!("{repo}/+/{}", hunk.oid)) { (&hunk.short_oid) }
                            }
                            @if let Some(parent) = &hunk.parent {
                                " " a href=(format!(
                                    "{repo}/+/{parent}/+/blame/{}",
                                    crate::endpoints::shared::encode_path(&hunk.original_path)
                                )) title="Blame the previous revision" { "^" }
                            }
                        }
                    }
                    td class=(blame::LINENUMBERS) { a id=(format!("n{line}")) href=(format!("#n{line}")) { (line) } }
                    td class=(blame::LINES) { pre { code { (lines.get(line - 1).map_or("", String::as_str)) } } }
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
                a href=(format!("{repository}/+/{revision}/+/blame/{}", crate::endpoints::shared::encode_path(&current))) { (part) }
            } @else {
                a href=(format!("{repository}/+/{revision}/+/tree/{}", crate::endpoints::shared::encode_path(&current))) { (part) }
            }
        }
    }
}
