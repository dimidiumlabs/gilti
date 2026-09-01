// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::styles::classes::table;
use maud::{Markup, Render, html};

/// The visual frame shared by repository listing tables.
pub enum TableFrame {
    List { nowrap: bool },
}

/// Visual behavior for a row in a [`TableFrame::List`].
pub enum RowStyle {
    Normal,
    Static,
    Highlighted,
    PreserveStripeOnHover,
}

/// A list row whose generated CSS class remains an implementation detail of the table component.
pub struct ListRow {
    pub style: RowStyle,
    pub content: Markup,
}

impl Render for ListRow {
    fn render(&self) -> Markup {
        let class = match self.style {
            RowStyle::Normal => None,
            RowStyle::Static => Some(table::STATIC_ROW),
            RowStyle::Highlighted => Some(table::HIGHLIGHTED_ROW),
            RowStyle::PreserveStripeOnHover => Some(table::PRESERVE_STRIPE_ON_HOVER),
        };
        html! { tr class=[class] { (self.content) } }
    }
}

/// A shared column grid for separate but visually aligned data tables.
pub struct TableGrid {
    pub content: Markup,
}

impl Render for TableGrid {
    fn render(&self) -> Markup {
        html! {
            div class=(table::GRID) { (&self.content) }
        }
    }
}

/// Domain-independent table frame for page-provided headers and rows.
pub struct DataTable<'a> {
    pub summary: Option<&'a str>,
    pub frame: TableFrame,
    pub content: Markup,
}

impl Render for DataTable<'_> {
    fn render(&self) -> Markup {
        let class = match self.frame {
            TableFrame::List { nowrap: true } => format!("{} {}", table::LIST, table::NOWRAP),
            TableFrame::List { nowrap: false } => table::LIST.to_owned(),
        };

        html! {
            table summary=[self.summary] class=(class) {
                (self.content)
            }
        }
    }
}
