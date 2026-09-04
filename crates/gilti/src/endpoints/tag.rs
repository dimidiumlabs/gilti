// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::{
    components::{
        key_value::{KeyValue, KeyValueList},
        relative_time::RelativeTime,
    },
    styles::classes::tag,
};

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::Revision>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let crate::router::Revision::Ref(reference) = route.params else {
        return super::error(gilti_git::Error::NotFound);
    };
    let repositories = std::sync::Arc::clone(&context.repositories);
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::tag::Tag::load(repositories.as_path(), &route.repo, reference)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(gilti_git::Error::Internal(error.to_string()));
        }
    };
    let content = Page {
        model: &model,
        archive_formats: &context.archive_formats,
    }
    .render();
    super::shared::render(
        context,
        &model.repository,
        &model.reference,
        super::shared::Page::Revision,
        content,
        &method,
    )
}

struct Page<'a> {
    model: &'a gilti_git::tag::Tag,
    archive_formats: &'a [gilti_git::archive::Format],
}
impl Render for Page<'_> {
    fn render(&self) -> Markup {
        content(self.model, self.archive_formats)
    }
}

pub fn content(
    model: &gilti_git::tag::Tag,
    archive_formats: &[gilti_git::archive::Format],
) -> Markup {
    let repo = crate::endpoints::shared::repository_url(&model.repository.name);
    let revision = crate::endpoints::shared::encode_path(&model.reference);
    let mut metadata = vec![KeyValue {
        key: "tag name",
        value: html! {
            (&model.name)
            @if model.annotated { " (" span class=(tag::OID) { (&model.oid) } ")" }
        },
    }];
    if let Some(timestamp) = model.timestamp {
        metadata.push(KeyValue {
            key: "tag date",
            value: html! { (RelativeTime { timestamp }) },
        });
    }
    if model.annotated {
        metadata.push(KeyValue {
            key: "tagged by",
            value: html! { (&model.tagger) " <" (&model.tagger_email) ">" },
        });
    }
    metadata.push(KeyValue {
        key: "tagged object",
        value: html! { span class=(tag::OID) {
            @for (index, target) in model.targets.iter().enumerate() {
                @if index > 0 { " → " }
                @if target.commit {
                    a href=(format!("{repo}/+/{}", target.oid)) { (&target.oid) }
                } @else {
                    a href=(format!("{repo}/+/object/{}", target.oid)) { (&target.oid) }
                }
            }
        } },
    });
    if model.downloadable && !archive_formats.is_empty() {
        metadata.push(KeyValue {
            key: "download",
            value: html! { span class=(tag::OID) {
                @for format in archive_formats {
                    a href=(format!("{repo}/+/{revision}/+/archive?format={format}")) { (format) }
                    " "
                }
            } },
        });
    }
    html! { div {
        (KeyValueList { label: "tag info", items: metadata })

        @if model.annotated {
            @let mut lines = model.message.splitn(2, '\n');

            div class=(tag::SUBJECT) { (lines.next().unwrap_or_default()) }
            div class=(tag::MESSAGE) { (lines.next().unwrap_or("No decsription")) }
        }
    } }
}
