// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, html};

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::Revision>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let crate::router::Revision::Ref(reference) = route.params else {
        return super::error(crate::models::Error::NotFound);
    };
    let repositories = context.repositories;
    let model = tokio::task::spawn_blocking(move || {
        crate::models::tag::Tag::load(std::path::Path::new(repositories), &route.repo, reference)
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
        &model.reference,
        super::shared::Page::Revision,
        content,
        &method,
    )
}

fn content(model: &crate::models::tag::Tag) -> Markup {
    let repo = super::shared::repository_url(&model.repository.name);
    let revision = super::shared::encode_path(&model.reference);
    html! {
        table class="commit-info" {
            tr { td { "tag name" } td { (&model.name) @if model.annotated { " (" (&model.oid) ")" } } }
            @if let Some(timestamp) = model.timestamp {
                tr { td { "tag date" } td { (super::shared::age(timestamp)) } }
            }
            @if model.annotated {
                tr { td { "tagged by" } td { (&model.tagger) " <" (&model.tagger_email) ">" } }
            }
            tr { td { "tagged object" } td class="oid" {
                @for (index, target) in model.targets.iter().enumerate() {
                    @if index > 0 { " → " }
                    @if target.commit {
                        a href=(format!("{repo}/+/{}", target.oid)) { (&target.oid) }
                    } @else {
                        a href=(format!("{repo}/+/object/{}", target.oid)) { (&target.oid) }
                    }
                }
            } }
            @if model.downloadable {
                tr { td { "download" } td class="oid" {
                    @for format in ["tar", "tar.gz", "tar.bz2", "tar.lz", "tar.xz", "tar.zst", "zip"] {
                        a href=(format!("{repo}/+/{revision}/+/archive?format={format}")) { (format) } br;
                    }
                } }
            }
        }
        @if model.annotated {
            @let mut lines = model.message.splitn(2, '\n');
            div class="commit-subject" { (lines.next().unwrap_or_default()) }
            div class="commit-msg" { (lines.next().unwrap_or_default()) }
        }
    }
}
