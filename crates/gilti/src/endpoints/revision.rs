// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::Revision>,
    query: super::diff::Query,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let name = route.repo.clone();
    let revision = route.params.clone();
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::commit::Commit::load(
            std::path::Path::new(repositories),
            &name,
            revision,
            query.options(),
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(gilti_git::Error::Internal(error.to_string())),
    };
    let content = Page {
        model: &model,
        query,
    }
    .render();
    super::shared::render_with_options(
        context,
        &model.repository,
        &model.revision,
        super::shared::Page::Revision,
        super::shared::RenderOptions {
            page_title: Some(&model.subject),
            sidebar: Some(crate::endpoints::diff::options_sidebar(query)),
            ..Default::default()
        },
        content,
        &method,
    )
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

// Route-specific presentation for the repository page.

use maud::{Markup, Render, html};

use crate::{
    components::key_value::{KeyValue, KeyValueList},
    styles::classes::revision,
};

struct Page<'a> {
    model: &'a gilti_git::commit::Commit,
    query: crate::endpoints::diff::Query,
}
impl Render for Page<'_> {
    fn render(&self) -> Markup {
        content(self.model, self.query)
    }
}

pub fn content(model: &gilti_git::commit::Commit, query: crate::endpoints::diff::Query) -> Markup {
    let repo = crate::endpoints::shared::repository_url(&model.repository.name);
    let revision = crate::endpoints::shared::encode_path(&model.revision);
    let patch_old = model.parents.first().map_or("HEAD", String::as_str);
    let mut metadata = vec![
        KeyValue {
            key: "author",
            value: html! {
                (&model.author.name) " <" (&model.author.email) "> " (timestamp(&model.author))
            },
        },
        KeyValue {
            key: "committer",
            value: html! {
                (&model.committer.name) " <" (&model.committer.email) "> " (timestamp(&model.committer))
            },
        },
        KeyValue {
            key: "commit",
            value: html! { span class=(revision::OID) {
                a href=(format!("{repo}/+/{}", model.oid)) { (&model.oid) }
                " (" a href=(format!("{repo}/+/patch/{patch_old}..{}", model.oid)) { "patch" } ")"
            } },
        },
        KeyValue {
            key: "tree",
            value: html! { span class=(revision::OID) {
                a href=(format!("{repo}/+/{revision}/+/tree")) { (&model.tree) }
            } },
        },
    ];
    metadata.extend(model.parents.iter().map(|parent| KeyValue {
        key: "parent",
        value: html! { span class=(revision::OID) {
            a href=(format!("{repo}/+/{parent}")) { (parent) }
            " (" a href=(format!("{repo}/+/diff/{parent}..{}", model.oid)) { "diff" } ")"
        } },
    }));
    metadata.push(KeyValue {
        key: "download",
        value: html! { span class=(revision::OID) {
            @for format in ["tar", "tar.gz", "tar.bz2", "tar.lz", "tar.xz", "tar.zst", "zip"] {
                a href=(format!("{repo}/+/{revision}/+/archive?format={format}")) { (format) }
                " "
            }
        } },
    });
    html! { div {
        (KeyValueList { label: "commit info", items: metadata })

        div class=(revision::SUBJECT) {
            (&model.subject)
            @for decoration in &model.decorations {
                " ("
                @if let Some(reference) = &decoration.reference {
                    @let reference = crate::endpoints::shared::encode_path(reference);
                    a href=(format!("{repo}/+/{reference}")) { (&decoration.label) }
                } @else { (&decoration.label) }
                ")"
            }
        }

        div class=(revision::MESSAGE) { (&model.message) }

        @if let Some(notes) = &model.notes {
            div class=(revision::NOTES_HEADER) { "Notes" }
            div class=(revision::NOTES) { (notes) }
            div class=(revision::NOTES_FOOTER) {}
        }

        @if let Some(diff) = &model.diff {
            (crate::components::diff::Diff { model: diff, mode: query.mode, path: None }.render())
        }
    } }
}

fn timestamp(identity: &gilti_git::commit::Identity) -> String {
    let adjusted = identity.timestamp + i64::from(identity.offset_minutes) * 60;
    let value = gilti_git::time::utc(adjusted)
        .unwrap_or_else(|| gilti_git::time::utc(0).expect("the Unix epoch is representable"));
    let sign = if identity.offset_minutes < 0 {
        '-'
    } else {
        '+'
    };
    let offset = identity.offset_minutes.unsigned_abs();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {sign}{:02}{:02}",
        value.tm_year + 1900,
        value.tm_mon + 1,
        value.tm_mday,
        value.tm_hour,
        value.tm_min,
        value.tm_sec,
        offset / 60,
        offset % 60
    )
}
