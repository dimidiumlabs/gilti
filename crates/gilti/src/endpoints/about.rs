// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<()>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::about::About::load(std::path::Path::new(repositories), &route.repo)
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(gilti_git::Error::Internal(error.to_string()));
        }
    };
    let readme = String::from_utf8_lossy(&model.bytes);
    let content = AboutPage { readme: &readme }.render();
    super::shared::render(
        context,
        &model.repository,
        "HEAD",
        super::shared::Page::About,
        content,
        &method,
    )
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

// Repository README presentation.

use maud::{Markup, Render, html};

struct AboutPage<'a> {
    pub readme: &'a str,
}

impl Render for AboutPage<'_> {
    fn render(&self) -> Markup {
        html! { div { pre { (self.readme) } } }
    }
}
