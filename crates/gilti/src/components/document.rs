// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{DOCTYPE, Markup, Render, html};

use crate::styles::classes::document;

/// A complete HTML document with the service's fixed browser assets.
pub struct Document<'a> {
    pub title: &'a str,
    pub body: Markup,
}

impl Render for Document<'_> {
    fn render(&self) -> Markup {
        html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    title { (self.title) }

                    meta name="generator" content="Gilti";
                    meta name="robots" content="index, nofollow";
                    link rel="shortcut icon" href="/-/assets/favicon.ico";

                    link rel="stylesheet" type="text/css" href="/-/assets/gilti.css";
                    script type="text/javascript" src="/-/assets/gilti.js" {}
                }

                body class=(document::ROOT) { (self.body) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use maud::{Render, html};

    use super::Document;

    #[test]
    fn escapes_document_titles_and_keeps_asset_url() {
        let document = Document {
            title: "<Gilti>",
            body: html! { p { "ok" } },
        }
        .render()
        .into_string();
        assert!(document.contains("&lt;Gilti&gt;"));
        assert!(document.contains("/-/assets/gilti.css"));
    }
}
