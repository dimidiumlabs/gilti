// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::styles::classes::header;

pub struct LinkLabel<'a> {
    pub url: &'a str,
    pub label: &'a str,
}
pub struct Header<'a> {
    pub home_url: &'a str,
    pub logo_url: &'a str,
    pub root_title: &'a str,
    pub repository: Option<LinkLabel<'a>>,
    pub description: &'a str,
}
impl Render for Header<'_> {
    fn render(&self) -> Markup {
        html! { table class=(header::ROOT) {
            tr { td class=(header::LOGO) rowspan="2" { a href=(self.home_url) { img src=(self.logo_url) alt="gilti logo"; } }
                td class=(header::MAIN) { (self.root_title) @if let Some(repository) = &self.repository { " : " a href=(repository.url) { (repository.label) } } } }
            tr { td class=(header::SUB) { (self.description) } }
        } }
    }
}
