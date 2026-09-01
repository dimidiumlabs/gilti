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
        html! { table summary=[self.summary] class=(class) { (self.content) } }
    }
}

#[cfg(test)]
mod tests {
    use maud::{Render, html};

    use super::{DataTable, ListRow, RowStyle, TableFrame};

    #[test]
    fn list_frame_and_rows_own_generated_classes() {
        let table = DataTable {
            summary: Some("rows"),
            frame: TableFrame::List { nowrap: true },
            content: html! {
                (ListRow { style: RowStyle::Static, content: html! { td { "static" } } })
                (ListRow { style: RowStyle::Highlighted, content: html! { td { "highlighted" } } })
                (ListRow { style: RowStyle::PreserveStripeOnHover, content: html! { td { "striped" } } })
                (ListRow { style: RowStyle::Normal, content: html! { td { "normal" } } })
            },
        }
        .render()
        .into_string();
        assert!(table.contains(crate::styles::classes::table::LIST));
        assert!(table.contains(crate::styles::classes::table::NOWRAP));
        assert!(table.contains(crate::styles::classes::table::STATIC_ROW));
        assert!(table.contains(crate::styles::classes::table::HIGHLIGHTED_ROW));
        assert!(table.contains(crate::styles::classes::table::PRESERVE_STRIPE_ON_HOVER));
        assert!(!table.contains("class=\"static-row\""));
    }
}
