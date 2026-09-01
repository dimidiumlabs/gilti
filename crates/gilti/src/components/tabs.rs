// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::styles::classes::tabs;

#[derive(Clone)]
pub struct Tab<'a> {
    pub url: &'a str,
    pub label: &'a str,
    pub active: bool,
}

#[derive(Clone)]
pub struct Tabs<'a> {
    pub items: Vec<Tab<'a>>,
    pub trailing: Option<Markup>,
}

impl Render for Tabs<'_> {
    fn render(&self) -> Markup {
        html! {
            table class=(tabs::ROOT) {
                tr {
                    td {
                        @for item in &self.items {
                            a href=(item.url) class=(format!("{}{}", tabs::ITEM, if item.active { format!(" {}", tabs::ACTIVE) } else { String::new() })) { (item.label) }
                        }
                    }
                    @if let Some(trailing) = &self.trailing {
                        td class=(tabs::FORM) { (trailing) }
                    }
                }
            }
        }
    }
}
