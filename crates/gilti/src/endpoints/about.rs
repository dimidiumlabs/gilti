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
    let repositories = std::sync::Arc::clone(&context.repositories);
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::about::About::load(repositories.as_path(), &route.repo)
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
    let content = AboutPage {
        readme: &readme,
        format: model.format,
    }
    .render();
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

use maud::{Markup, PreEscaped, Render, html};
use pulldown_cmark::{Event, Options, Parser, html as cmark_html};

struct AboutPage<'a> {
    pub readme: &'a str,
    pub format: gilti_git::about::ReadmeFormat,
}

impl Render for AboutPage<'_> {
    fn render(&self) -> Markup {
        html! { div {
            @match self.format {
                gilti_git::about::ReadmeFormat::Markdown => (render_markdown(self.readme)),
                gilti_git::about::ReadmeFormat::PlainText => pre { (self.readme) },
            }
        } }
    }
}

fn render_markdown(source: &str) -> Markup {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let events = Parser::new_ext(source, options).map(|event| match event {
        Event::Html(html) | Event::InlineHtml(html) => Event::Text(html),
        event => event,
    });
    let mut output = String::new();
    cmark_html::push_html(&mut output, events);
    PreEscaped(output)
}

#[cfg(test)]
mod tests {
    use maud::Render;

    use super::AboutPage;

    #[test]
    fn renders_markdown_as_semantic_html_without_trusting_raw_html() {
        let rendered = AboutPage {
            readme: "# Project\n\nA **useful** project.\n\n<script>alert(1)</script>",
            format: gilti_git::about::ReadmeFormat::Markdown,
        }
        .render()
        .into_string();

        assert!(rendered.contains("<h1>Project</h1>"));
        assert!(rendered.contains("<p>A <strong>useful</strong> project.</p>"));
        assert!(rendered.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!rendered.contains("<script>"));
    }

    #[test]
    fn preserves_plain_readmes_as_escaped_preformatted_text() {
        let rendered = AboutPage {
            readme: "Project\n<b>plain text</b>",
            format: gilti_git::about::ReadmeFormat::PlainText,
        }
        .render()
        .into_string();

        assert!(rendered.contains("<pre>Project\n&lt;b&gt;plain text&lt;/b&gt;</pre>"));
    }
}
