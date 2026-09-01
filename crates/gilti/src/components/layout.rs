// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::styles::classes::layout;
use maud::{Markup, Render, html};

pub struct ContentLayout {
    pub content: Markup,
    pub footer: Markup,
}
impl Render for ContentLayout {
    fn render(&self) -> Markup {
        html! { div { div class=(layout::CONTENT) { (self.content) } div class=(layout::FOOTER) { (self.footer) } } }
    }
}
