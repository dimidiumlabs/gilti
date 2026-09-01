// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::styles::classes::key_value;

pub struct KeyValue<'a> {
    pub key: &'a str,
    pub value: Markup,
}

pub struct KeyValueList<'a> {
    pub label: &'a str,
    pub items: Vec<KeyValue<'a>>,
}

impl Render for KeyValueList<'_> {
    fn render(&self) -> Markup {
        html! {
            dl class=(key_value::ROOT) aria-label=(self.label) {
                @for item in &self.items {
                    dt { (item.key) }
                    dd { (&item.value) }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use maud::{Render, html};

    use super::{KeyValue, KeyValueList};

    #[test]
    fn renders_a_semantic_definition_list() {
        let rendered = KeyValueList {
            label: "object info",
            items: vec![KeyValue {
                key: "object",
                value: html! { code { "abc123" } },
            }],
        }
        .render()
        .into_string();

        assert!(rendered.starts_with("<dl "));
        assert!(rendered.contains("<dt>object</dt><dd><code>abc123</code></dd>"));
        assert!(!rendered.contains("<table"));
    }
}
