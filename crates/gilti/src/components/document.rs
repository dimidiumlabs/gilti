// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

pub fn render(title: &str, body: Markup) -> Markup {
    dimidiumlabs_ui::Document::new(title, body)
        .with_script()
        .with_manifest()
        .with_svg_icon()
        .with_apple_touch_icon()
        .with_head(html! { meta name="generator" content="Gilti"; })
        .render()
}

#[cfg(test)]
mod tests {
    use maud::html;

    #[test]
    fn includes_gilti_generator_metadata() {
        let document = super::render("Gilti", html! { main { "content" } }).into_string();
        assert!(document.contains("<meta name=\"generator\" content=\"Gilti\">"));
    }
}
