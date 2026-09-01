// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use super::{
    relative_time::RelativeTime,
    table::{DataTable, ListRow, RowStyle, TableFrame},
};
use crate::styles::classes::refs_table;

pub struct BranchesTable<'a> {
    pub repository_url: &'a str,
    pub branches: &'a [gilti_git::refs::Branch],
    pub nowrap: bool,
}

impl Render for BranchesTable<'_> {
    fn render(&self) -> Markup {
        html! {
            (DataTable {
                summary: Some("branches"),
                frame: TableFrame::List { nowrap: self.nowrap },
                content: html! {
                    (ListRow { style: RowStyle::Static, content: html! {
                        th class=(refs_table::LEFT) { "Branch" }
                        th class=(refs_table::LEFT) { "Commit message" }
                        th class=(refs_table::LEFT) { "Author" }
                        th class=(refs_table::LEFT) { "Age" }
                    } })
                    @for branch in self.branches {
                        @let revision = crate::urls::encode_path(&branch.reference);
                        tr {
                            td { a href=(format!("{}/+/{revision}/+/log", self.repository_url)) { (&branch.name) } }
                            td { a href=(format!("{}/+/{revision}", self.repository_url)) { (&branch.subject) } }
                            td { (&branch.author) }
                            td { (RelativeTime { timestamp: branch.timestamp }) }
                        }
                    }
                },
            })
        }
    }
}

pub struct TagsTable<'a> {
    pub repository_url: &'a str,
    pub tags: &'a [gilti_git::refs::Tag],
    pub nowrap: bool,
}

impl Render for TagsTable<'_> {
    fn render(&self) -> Markup {
        html! {
            (DataTable {
                summary: Some("tags"),
                frame: TableFrame::List { nowrap: self.nowrap },
                content: html! {
                    (ListRow { style: RowStyle::Static, content: html! {
                        th class=(refs_table::LEFT) { "Tag" }
                        th class=(refs_table::LEFT) { "Download" }
                        th class=(refs_table::LEFT) { "Author" }
                        th class=(refs_table::LEFT) colspan="2" { "Age" }
                    } })
                    @for tag in self.tags {
                        @let revision = crate::urls::encode_path(&tag.reference);
                        tr {
                            td { a href=(format!("{}/+/{revision}", self.repository_url)) { (&tag.name) } }
                            td {
                                @if tag.downloadable {
                                    @for (index, format) in ["tar", "tar.gz", "tar.bz2", "tar.lz", "tar.xz", "tar.zst", "zip"].iter().enumerate() {
                                        @if index > 0 { "  " }
                                        a href=(format!("{}/+/{revision}/+/archive?format={format}", self.repository_url)) { (format) }
                                    }
                                } @else {
                                    a href=(format!("{}/+/object/{}", self.repository_url, tag.target)) { (&tag.target) }
                                }
                            }
                            td { (&tag.author) }
                            td colspan="2" { (RelativeTime { timestamp: tag.timestamp }) }
                        }
                    }
                },
            })
        }
    }
}
