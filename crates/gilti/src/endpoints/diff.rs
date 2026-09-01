// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

#![allow(clippy::items_after_test_module)]

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Mode {
    Unified,
    SideBySide,
    StatOnly,
}

#[derive(Clone, Copy)]
pub struct Query {
    pub context: u32,
    pub ignore_whitespace: bool,
    pub mode: Mode,
}

impl Query {
    pub fn from_request(query: &crate::RequestQuery) -> Result<Self, ()> {
        let context = query
            .value("GILTI_QUERY_CONTEXT")
            .map(str::parse)
            .transpose()
            .map_err(|_| ())?
            .unwrap_or(3);
        let context = if context == 0 { 3 } else { context };
        if context > 40 {
            return Err(());
        }
        let ignore_whitespace = match query.value("GILTI_QUERY_IGNOREWS") {
            None | Some("0") => false,
            Some("1") => true,
            _ => return Err(()),
        };
        let mode = match query.value("GILTI_QUERY_DIFFTYPE") {
            None | Some("0") => Mode::Unified,
            Some("1") => Mode::SideBySide,
            Some("2") => Mode::StatOnly,
            _ => return Err(()),
        };
        Ok(Self {
            context,
            ignore_whitespace,
            mode,
        })
    }

    pub fn options(self) -> gilti_git::diff::Options {
        gilti_git::diff::Options {
            context: self.context,
            ignore_whitespace: self.ignore_whitespace,
        }
    }
}

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::Comparison>,
    query: Query,
    raw: bool,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let name = route.repo.clone();
    let old = route.params.old_rev.clone();
    let new = route.params.new_rev.clone();
    let path = route.params.path.clone();
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::diff::Diff::load(
            std::path::Path::new(repositories),
            &name,
            Some(old),
            new,
            path,
            query.options(),
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(gilti_git::Error::Internal(error.to_string())),
    };
    if raw {
        return raw_response(context, &route, &model, query, &method).await;
    }
    let revision = model.new_revision.clone();
    let content = crate::endpoints::diff::DiffPage {
        model: &model,
        query,
        path: route.params.path.as_deref(),
        controls: true,
    }
    .render();
    super::shared::render(
        context,
        &model.repository,
        &revision,
        super::shared::Page::Diff,
        content,
        &method,
    )
}

async fn raw_response(
    context: &super::shared::Context,
    route: &crate::router::RepoRoute<crate::router::Comparison>,
    model: &gilti_git::diff::Diff,
    query: Query,
    method: &axum::http::Method,
) -> axum::response::Response {
    let repository = match gilti_git::repository::path(
        std::path::Path::new(context.repositories),
        &route.repo,
    ) {
        Ok(repository) => repository,
        Err(error) => return super::error(error),
    };
    let output = match gilti_git::commands::raw_diff(
        &repository,
        model.old_oid.as_deref(),
        &model.new_oid,
        route.params.path.as_deref(),
        query.context,
        query.ignore_whitespace,
    )
    .await
    {
        Ok(output) => output,
        Err(error) => return super::error(error),
    };
    super::bytes_response("text/plain; charset=UTF-8", None, None, output, method)
}

#[cfg(test)]
mod tests {
    #[test]
    fn highlights_side_by_side_character_changes() {
        let common = crate::endpoints::diff::common_subsequence("alpha", "aloha");
        assert_eq!(common, ['a', 'l', 'h', 'a']);
        assert_eq!(
            crate::endpoints::diff::highlighted_segments("alpha", &common),
            [
                (false, "al".to_owned()),
                (true, "p".to_owned()),
                (false, "ha".to_owned()),
            ]
        );
    }
}
// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::styles::classes::diff;

/// Presentation page for a repository comparison.
pub(crate) struct DiffPage<'a> {
    pub model: &'a gilti_git::diff::Diff,
    pub query: Query,
    pub path: Option<&'a str>,
    pub controls: bool,
}
impl Render for DiffPage<'_> {
    fn render(&self) -> Markup {
        render_content(self.model, self.query, self.path, self.controls)
    }
}

fn render_content(
    model: &gilti_git::diff::Diff,
    query: Query,
    path: Option<&str>,
    controls: bool,
) -> Markup {
    html! {
        @if controls { (content_controls(query)) }
        div class=(diff::DIFFSTAT_HEADER) {
            "Diffstat"
            @if let Some(path) = path { " (limited to '" (path) "')" }
        }
        table summary="diffstat" class=(diff::DIFFSTAT) {
            @for file in &model.files { (file_stat(model, file)) }
        }
        div class=(diff::DIFFSTAT_SUMMARY) {
            (model.files.len()) " files changed, " (model.additions) " insertions, " (model.deletions) " deletions"
        }
        @if query.mode != Mode::StatOnly {
            @if query.mode == Mode::SideBySide {
                table summary="ssdiff" class=(diff::SSDIFF) {
                    @for file in &model.files { (side_file(model, file)) }
                }
            } @else {
                table summary="diff" class=(diff::DIFF) { tr { td {
                    @for file in &model.files { (unified_file(model, file)) }
                } } }
            }
        }
    }
}

pub fn content_controls(query: Query) -> Markup {
    html! { div class=(diff::GILTI_PANEL) {
        b { "diff options" }
        form method="get" { table {
            tr { td colspan="2" {} }
            tr { td class=(diff::LABEL) { "context:" } td class=(diff::CTRL) {
                select name="context" onchange="this.form.submit();" {
                    @for value in [1_u32,2,3,4,5,6,7,8,9,10,15,20,25,30,35,40] {
                        option value=(value) selected[query.context == value] { (value) }
                    }
                }
            } }
            tr { td class=(diff::LABEL) { "space:" } td class=(diff::CTRL) {
                select name="ignorews" onchange="this.form.submit();" {
                    option value="0" selected[!query.ignore_whitespace] { "include" }
                    option value="1" selected[query.ignore_whitespace] { "ignore" }
                }
            } }
            tr { td class=(diff::LABEL) { "mode:" } td class=(diff::CTRL) {
                select name="dt" onchange="this.form.submit();" {
                    option value="0" selected[query.mode == Mode::Unified] { "unified" }
                    option value="1" selected[query.mode == Mode::SideBySide] { "ssdiff" }
                    option value="2" selected[query.mode == Mode::StatOnly] { "stat only" }
                }
            } }
            tr { td {} td class=(diff::CTRL) { noscript { input type="submit" value="reload"; } } }
        } }
    } }
}

fn file_stat(model: &gilti_git::diff::Diff, file: &gilti_git::diff::File) -> Markup {
    let path = if file.new_path.is_empty() {
        &file.old_path
    } else {
        &file.new_path
    };
    let repo = crate::endpoints::shared::repository_url(&model.repository.name);
    let old =
        crate::endpoints::shared::encode_path(model.old_revision.as_deref().unwrap_or("HEAD"));
    let new = crate::endpoints::shared::encode_path(&model.new_revision);
    let path_url = crate::endpoints::shared::encode_path(path);
    let class = status_class(file.status);
    html! { tr {
        td class=(diff::MODE) {
            (crate::endpoints::tree::filemode(if file.new_mode == 0 { file.old_mode } else { file.new_mode }))
            @if file.old_mode != 0 && file.new_mode != 0 && file.old_mode != file.new_mode {
                span class=(diff::MODECHANGE) { "[" (crate::endpoints::tree::filemode(file.old_mode)) "]" }
            }
        }
        td class=(class) {
            a href=(format!("{repo}/+/diff/{old}..{new}/+/{path_url}")) { (path) }
            @if matches!(file.status, gilti_git::diff::Status::Renamed | gilti_git::diff::Status::Copied) {
                " (" @if file.status == gilti_git::diff::Status::Copied { "copied" } @else { "renamed" }
                " from " (&file.old_path) ")"
            }
        }
        td class=(diff::RIGHT) {
            @if file.binary { "bin" } @else { (file.additions + file.deletions) }
        }
        td class=(diff::GRAPH) {
            @if file.binary { (file.old_size) " -> " (file.new_size) " bytes" }
            @else { "+" (file.additions) " −" (file.deletions) }
        }
    } }
}

fn unified_file(model: &gilti_git::diff::Diff, file: &gilti_git::diff::File) -> Markup {
    html! {
        (file_header(model, file))
        @if file.binary { div class=(diff::CTX) { "Binary files differ" } }
        @for hunk in &file.hunks {
            div class=(diff::HUNK) { (&hunk.header) }
            @for line in &hunk.lines {
                div class=(line_class(line.origin)) {
                    @if matches!(line.origin, '+' | '-' | ' ') { (line.origin) }
                    (&line.content)
                }
            }
        }
    }
}

fn side_file(model: &gilti_git::diff::Diff, file: &gilti_git::diff::File) -> Markup {
    html! {
        tr { td colspan="4" { (file_header(model, file)) } }
        @if file.binary { tr { td colspan="4" { "Binary files differ" } } }
        @for hunk in &file.hunks {
            tr { td colspan="4" class=(diff::HUNK) { (&hunk.header) } }
            @for (old, new) in side_rows(hunk) { tr {
                td class=(diff::LINENO) { @if let Some(line) = old { (line.old_line.unwrap_or_default()) } }
                td class=[old.map(|line| line_class(line.origin))] { (side_content(old, new, true)) }
                td class=(diff::LINENO) { @if let Some(line) = new { (line.new_line.unwrap_or_default()) } }
                td class=[new.map(|line| line_class(line.origin))] { (side_content(new, old, false)) }
            } }
        }
    }
}

fn side_rows(
    hunk: &gilti_git::diff::Hunk,
) -> Vec<(
    Option<&gilti_git::diff::Line>,
    Option<&gilti_git::diff::Line>,
)> {
    let mut rows = Vec::new();
    let mut removed = Vec::new();
    let mut added = Vec::new();
    let flush = |rows: &mut Vec<_>, removed: &mut Vec<_>, added: &mut Vec<_>| {
        let count = removed.len().max(added.len());
        for index in 0..count {
            rows.push((removed.get(index).copied(), added.get(index).copied()));
        }
        removed.clear();
        added.clear();
    };
    for line in &hunk.lines {
        match line.origin {
            '-' => removed.push(line),
            '+' => added.push(line),
            _ => {
                flush(&mut rows, &mut removed, &mut added);
                rows.push((Some(line), Some(line)));
            }
        }
    }
    flush(&mut rows, &mut removed, &mut added);
    rows
}

fn side_content(
    line: Option<&gilti_git::diff::Line>,
    other: Option<&gilti_git::diff::Line>,
    old: bool,
) -> Markup {
    let Some(line) = line else {
        return html! {};
    };
    let paired_change =
        other.is_some_and(|other| matches!((line.origin, other.origin), ('-', '+') | ('+', '-')));
    if !paired_change {
        return html! { (&line.content) };
    }
    let other = &other.expect("paired line exists").content;
    let common = if old {
        common_subsequence(&line.content, other)
    } else {
        common_subsequence(other, &line.content)
    };
    let segments = highlighted_segments(&line.content, &common);
    let class = if old { diff::DEL } else { diff::ADD };
    html! {
        @for (changed, text) in segments {
            @if changed { span class=(class) { (text) } } @else { (text) }
        }
    }
}

pub fn common_subsequence(old: &str, new: &str) -> Vec<char> {
    let old = old.chars().collect::<Vec<_>>();
    let new = new.chars().collect::<Vec<_>>();
    if old.len() >= 200 || new.len() >= 200 {
        return Vec::new();
    }
    let mut lengths = vec![vec![0_usize; new.len() + 1]; old.len() + 1];
    for left in (0..old.len()).rev() {
        for right in (0..new.len()).rev() {
            lengths[left][right] = if old[left] == new[right] {
                lengths[left + 1][right + 1] + 1
            } else {
                lengths[left + 1][right].max(lengths[left][right + 1])
            };
        }
    }
    let (mut left, mut right) = (0, 0);
    let mut common = Vec::with_capacity(lengths[0][0]);
    while left < old.len() && right < new.len() {
        if old[left] == new[right] {
            common.push(old[left]);
            left += 1;
            right += 1;
        } else if lengths[left + 1][right] >= lengths[left][right + 1] {
            left += 1;
        } else {
            right += 1;
        }
    }
    common
}

pub fn highlighted_segments(value: &str, common: &[char]) -> Vec<(bool, String)> {
    let mut common = common.iter().copied().peekable();
    let mut segments = Vec::<(bool, String)>::new();
    for character in value.chars() {
        let changed = common.peek().copied() != Some(character);
        if !changed {
            common.next();
        }
        if let Some((last_changed, text)) = segments.last_mut()
            && *last_changed == changed
        {
            text.push(character);
            continue;
        }
        segments.push((changed, character.to_string()));
    }
    segments
}

fn file_header(model: &gilti_git::diff::Diff, file: &gilti_git::diff::File) -> Markup {
    let old = if file.old_path.is_empty() {
        "/dev/null"
    } else {
        &file.old_path
    };
    let new = if file.new_path.is_empty() {
        "/dev/null"
    } else {
        &file.new_path
    };
    html! { div class=(diff::HEAD) {
        "diff --git a/" (old) " b/" (new)
        @if file.old_mode == 0 { br; "new file mode " (format!("{:06o}", file.new_mode)) }
        @if file.new_mode == 0 { br; "deleted file mode " (format!("{:06o}", file.old_mode)) }
        @if let (Some(old_oid), Some(new_oid)) = (&file.old_oid, &file.new_oid) {
            br; "index " (&old_oid[..old_oid.len().min(7)]) ".." (&new_oid[..new_oid.len().min(7)])
            @if file.old_mode != 0 && file.new_mode != 0 {
                " " (format!("{:06o}", file.old_mode))
                @if file.old_mode != file.new_mode { ".." (format!("{:06o}", file.new_mode)) }
            }
        }
        br; "--- " @if file.old_mode == 0 { "/dev/null" } @else { "a/" (old) }
        br; "+++ " @if file.new_mode == 0 { "/dev/null" } @else { "b/" (new) }
        @if model.old_oid.is_none() { "" }
    } }
}

fn line_class(origin: char) -> &'static str {
    match origin {
        '+' => diff::ADD,
        '-' => diff::DEL,
        '@' => diff::HUNK,
        _ => diff::CTX,
    }
}

fn status_class(status: gilti_git::diff::Status) -> &'static str {
    match status {
        gilti_git::diff::Status::Added => diff::ADD,
        gilti_git::diff::Status::Copied => diff::CPY,
        gilti_git::diff::Status::Deleted => diff::DEL,
        gilti_git::diff::Status::Modified => diff::UPD,
        gilti_git::diff::Status::Renamed => diff::MOV,
        gilti_git::diff::Status::Typechange => diff::TYP,
        gilti_git::diff::Status::Conflicted => diff::STG,
        _ => diff::UNK,
    }
}
