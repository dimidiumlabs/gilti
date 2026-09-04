// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::styles::classes::diff;

use super::code_block::{CodeBlock, CodeLine, LineNumbers, LineStyle};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Mode {
    Unified,
    SideBySide,
    StatOnly,
}

/// Presentation page for a repository comparison.
pub struct Diff<'a> {
    pub model: &'a gilti_git::diff::Diff,
    pub mode: Mode,
    pub path: Option<&'a str>,
    pub abbreviated_oid_chars: usize,
}
impl Render for Diff<'_> {
    fn render(&self) -> Markup {
        render_content(self.model, self.mode, self.path, self.abbreviated_oid_chars)
    }
}

fn render_content(
    model: &gilti_git::diff::Diff,
    mode: Mode,
    path: Option<&str>,
    abbreviated_oid_chars: usize,
) -> Markup {
    html! {
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
        @if mode != Mode::StatOnly {
            @if mode == Mode::SideBySide {
                table summary="ssdiff" class=(diff::SSDIFF) {
                    @for file in &model.files { (side_file(model, file, abbreviated_oid_chars)) }
                }
            } @else {
                @for (index, file) in model.files.iter().enumerate() {
                    (unified_file(model, file, index, abbreviated_oid_chars))
                }
            }
        }
    }
}

fn file_stat(model: &gilti_git::diff::Diff, file: &gilti_git::diff::File) -> Markup {
    let path = if file.new_path.is_empty() {
        &file.old_path
    } else {
        &file.new_path
    };
    let repo = crate::urls::repository(&model.repository.name);
    let old = crate::urls::encode_path(model.old_revision.as_deref().unwrap_or("HEAD"));
    let new = crate::urls::encode_path(&model.new_revision);
    let path_url = crate::urls::encode_path(path);
    let class = status_class(file.status);
    html! { tr {
        td class=(diff::MODE) {
            (crate::components::file_mode(if file.new_mode == 0 { file.old_mode } else { file.new_mode }))
            @if file.old_mode != 0 && file.new_mode != 0 && file.old_mode != file.new_mode {
                span class=(diff::MODECHANGE) { "[" (crate::components::file_mode(file.old_mode)) "]" }
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

fn unified_file(
    model: &gilti_git::diff::Diff,
    file: &gilti_git::diff::File,
    file_index: usize,
    abbreviated_oid_chars: usize,
) -> Markup {
    let mut lines = Vec::new();
    if file.binary {
        lines.push(CodeLine {
            anchor: format!("diff-{file_index}-binary"),
            old_number: None,
            new_number: None,
            annotation: None,
            content: html! { "Binary files differ" },
            style: LineStyle::Context,
        });
    }
    for (hunk_index, hunk) in file.hunks.iter().enumerate() {
        lines.push(CodeLine {
            anchor: format!("diff-{file_index}-hunk-{hunk_index}"),
            old_number: None,
            new_number: None,
            annotation: None,
            content: html! { (&hunk.header) },
            style: LineStyle::Hunk,
        });
        for (line_index, line) in hunk.lines.iter().enumerate() {
            lines.push(CodeLine {
                anchor: format!("diff-{file_index}-{hunk_index}-{line_index}"),
                old_number: line.old_line,
                new_number: line.new_line,
                annotation: None,
                content: html! {
                    @if matches!(line.origin, '+' | '-' | ' ') { (line.origin) }
                    (&line.content)
                },
                style: match line.origin {
                    '+' => LineStyle::Addition,
                    '-' => LineStyle::Deletion,
                    '@' => LineStyle::Hunk,
                    _ => LineStyle::Context,
                },
            });
        }
    }
    html! {
        (file_header(model, file, abbreviated_oid_chars))
        (CodeBlock {
            summary: "diff content",
            numbers: LineNumbers::Diff,
            annotations: false,
            lines,
        })
    }
}

fn side_file(
    model: &gilti_git::diff::Diff,
    file: &gilti_git::diff::File,
    abbreviated_oid_chars: usize,
) -> Markup {
    html! {
        tr { td colspan="4" { (file_header(model, file, abbreviated_oid_chars)) } }
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

fn file_header(
    model: &gilti_git::diff::Diff,
    file: &gilti_git::diff::File,
    abbreviated_oid_chars: usize,
) -> Markup {
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
            br; "index " (&old_oid[..old_oid.len().min(abbreviated_oid_chars)]) ".." (&new_oid[..new_oid.len().min(abbreviated_oid_chars)])
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
