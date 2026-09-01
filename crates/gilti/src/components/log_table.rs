// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use super::{
    relative_time::RelativeTime,
    table::{DataTable, ListRow, RowStyle, TableFrame},
};
use crate::styles::classes::log_table;

pub enum LogTable<'a> {
    Summary {
        repository_url: &'a str,
        commits: &'a [gilti_git::overview::Commit],
    },
    History {
        model: &'a gilti_git::history::History,
        show_message: bool,
        expand_url: String,
        branch_suffix: String,
    },
}

impl Render for LogTable<'_> {
    fn render(&self) -> Markup {
        match self {
            Self::Summary {
                repository_url,
                commits,
            } => render_summary(repository_url, commits),
            Self::History {
                model,
                show_message,
                expand_url,
                branch_suffix,
            } => render_history(model, *show_message, expand_url, branch_suffix),
        }
    }
}

fn render_summary(repository_url: &str, commits: &[gilti_git::overview::Commit]) -> Markup {
    html! {
        (DataTable {
            summary: Some("recent commits"),
            frame: TableFrame::List { nowrap: false },
            content: html! {
                (ListRow { style: RowStyle::Static, content: html! {
                    th class=(log_table::LEFT) { "Lines" }
                    th class=(log_table::LEFT) { "Commit message" }
                    th class=(log_table::LEFT) { "Author" }
                    th class=(log_table::LEFT) { "Age" }
                } })
                @for commit in commits {
                    tr {
                        td {
                            span class=(log_table::DELETIONS) { "-" (commit.deletions) }
                            "/"
                            span class=(log_table::INSERTIONS) { "+" (commit.insertions) }
                        }
                        td {
                            a href=(format!("{repository_url}/+/{}", commit.oid)) { (&commit.subject) }
                            @if !commit.decorations.is_empty() {
                                span class=(log_table::DECORATION) {
                                    @for decoration in &commit.decorations {
                                        @if let Some(reference) = &decoration.reference {
                                            @let revision = crate::urls::encode_path(reference);
                                            a
                                                class=(if decoration.tag {
                                                    log_table::TAG_ANNOTATED_DECO
                                                } else {
                                                    log_table::BRANCH_DECO
                                                })
                                                href=(if decoration.tag {
                                                    format!("{repository_url}/+/{revision}")
                                                } else {
                                                    format!("{repository_url}/+/{revision}/+/log")
                                                })
                                            { (&decoration.label) }
                                        } @else {
                                            a class=(log_table::DECO) href=(format!("{repository_url}/+/{}", commit.oid)) {
                                                (&decoration.label)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        td { (&commit.author) }
                        td { (RelativeTime { timestamp: commit.timestamp }) }
                    }
                }
            },
        })
    }
}

fn render_history(
    model: &gilti_git::history::History,
    show_message: bool,
    expand_url: &str,
    branch_suffix: &str,
) -> Markup {
    let repository_url = crate::urls::repository(&model.repository.name);
    let columns = if model.graph { 6 } else { 5 };
    html! {
        (DataTable {
            summary: Some("commit history"),
            frame: TableFrame::List { nowrap: true },
            content: html! {
                (ListRow { style: RowStyle::Static, content: html! {
                    @if model.graph { th {} } @else { th class=(log_table::LEFT) { "Age" } }
                    th class=(log_table::LEFT) {
                        "Commit message (" a href=(expand_url) {
                            (if show_message { "Collapse" } else { "Expand" })
                        } ")"
                    }
                    th class=(log_table::LEFT) { "Author" }
                    @if model.graph { th class=(log_table::LEFT) { "Age" } }
                    th class=(log_table::LEFT) { "Files" }
                    th class=(log_table::LEFT) { "Lines" }
                } })
                @for entry in &model.entries {
                    @if model.graph {
                        @for continuation in &entry.graph_continuations {
                            (ListRow { style: RowStyle::Static, content: html! {
                                td class=(log_table::COMMIT_GRAPH) { (graph(continuation)) }
                                td colspan=(columns - 1) {}
                            } })
                        }
                    }
                    (ListRow {
                        style: if show_message { RowStyle::Highlighted } else { RowStyle::Normal },
                        content: html! {
                            @if model.graph {
                                td class=(log_table::COMMIT_GRAPH) { (graph(&entry.graph)) }
                            } @else {
                                td { (age(&entry.committer)) }
                            }
                            td class=[show_message.then_some(log_table::LOG_SUBJECT)] {
                                a href=(format!("{repository_url}/+/{}", entry.oid)) { (&entry.subject) }
                                @for decoration in &entry.decorations {
                                    @if let Some(reference) = &decoration.reference {
                                        @if reference.starts_with("refs/tags/") {
                                            span class=(log_table::DECORATION) {
                                                " " a class=(log_table::TAG_DECO) href=(format!("{repository_url}/+/{}", crate::urls::encode_path(reference))) {
                                                    (&decoration.label)
                                                }
                                            }
                                        } @else {
                                            @let link = entry.path.as_ref().map_or_else(
                                                || format!("{repository_url}/+/{}/+/log{branch_suffix}", crate::urls::encode_path(reference)),
                                                |path| format!("{repository_url}/+/{}/+/log/{}{branch_suffix}", crate::urls::encode_path(reference), crate::urls::encode_path(path)),
                                            );
                                            span class=(log_table::DECORATION) {
                                                " " a class=(log_table::BRANCH_DECO) href=(link) { (&decoration.label) }
                                            }
                                        }
                                    } @else {
                                        span class=(log_table::DECORATION) { " " (&decoration.label) }
                                    }
                                }
                            }
                            td { (&entry.author.name) }
                            @if model.graph { td { (age(&entry.committer)) } }
                            td { (entry.files) }
                            td {
                                span class=(log_table::DELETIONS) { "-" (entry.deletions) }
                                "/"
                                span class=(log_table::INSERTIONS) { "+" (entry.additions) }
                            }
                        },
                    })
                    @if show_message {
                        (ListRow { style: RowStyle::PreserveStripeOnHover, content: html! {
                            td colspan=(columns) class=(log_table::LOG_MESSAGE) {
                                (&entry.body)
                                @if let Some(notes) = &entry.notes { "\n" (notes) }
                            }
                        } })
                    }
                }
            },
        })
    }
}

fn graph(line: &str) -> Markup {
    html! {
        @for (index, character) in line.chars().enumerate() {
            span class=(graph_class(index / 2 % 6 + 1)) { (character) }
        }
    }
}

fn graph_class(column: usize) -> &'static str {
    [
        log_table::COLUMN1,
        log_table::COLUMN2,
        log_table::COLUMN3,
        log_table::COLUMN4,
        log_table::COLUMN5,
        log_table::COLUMN6,
    ][column - 1]
}

fn age(identity: &gilti_git::commit::Identity) -> Markup {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs().min(i64::MAX as u64) as i64);
    if now.saturating_sub(identity.timestamp) < 14 * 24 * 60 * 60 {
        return RelativeTime {
            timestamp: identity.timestamp,
        }
        .render();
    }
    let local = identity.timestamp + i64::from(identity.offset_minutes) * 60;
    let Some(value) = gilti_git::time::utc(local) else {
        return RelativeTime {
            timestamp: identity.timestamp,
        }
        .render();
    };
    html! {
        (format!(
            "{:04}-{:02}-{:02}",
            value.tm_year + 1900,
            value.tm_mon + 1,
            value.tm_mday,
        ))
    }
}
