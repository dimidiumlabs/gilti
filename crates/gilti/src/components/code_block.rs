// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::styles::classes::code_block;

#[derive(Clone, Copy)]
pub enum LineNumbers {
    Single,
    Diff,
}

#[derive(Clone, Copy)]
pub enum LineStyle {
    Context,
    Addition,
    Deletion,
    Hunk,
    Alternate,
}

pub struct CodeLine {
    pub anchor: String,
    pub old_number: Option<u32>,
    pub new_number: Option<u32>,
    pub annotation: Option<Markup>,
    pub content: Markup,
    pub style: LineStyle,
}

pub struct CodeBlock<'a> {
    pub summary: &'a str,
    pub numbers: LineNumbers,
    pub annotations: bool,
    pub lines: Vec<CodeLine>,
}

impl Render for CodeBlock<'_> {
    fn render(&self) -> Markup {
        html! {
            table summary=(self.summary) class=(code_block::ROOT) {
                @for line in &self.lines {
                    tr id=(&line.anchor) class=(line.style.class()) {
                        @if self.annotations {
                            td class=(code_block::ANNOTATION) {
                                @if let Some(annotation) = &line.annotation { (annotation) }
                            }
                        }
                        @match self.numbers {
                            LineNumbers::Single => {
                                (line_number(&line.anchor, line.new_number.or(line.old_number)))
                            }
                            LineNumbers::Diff => {
                                (line_number(&line.anchor, line.old_number))
                                (line_number(&line.anchor, line.new_number))
                            }
                        }
                        td class=(code_block::CODE) { pre { code { (&line.content) } } }
                    }
                }
            }
        }
    }
}

impl LineStyle {
    fn class(self) -> &'static str {
        match self {
            Self::Context => code_block::CONTEXT,
            Self::Addition => code_block::ADDITION,
            Self::Deletion => code_block::DELETION,
            Self::Hunk => code_block::HUNK,
            Self::Alternate => code_block::ALTERNATE,
        }
    }
}

fn line_number(anchor: &str, number: Option<u32>) -> Markup {
    html! {
        td class=(code_block::LINE_NUMBER) {
            @if let Some(number) = number {
                a href=(format!("#{anchor}")) { (number) }
            }
        }
    }
}

pub fn text_lines(text: &str) -> Vec<CodeLine> {
    text.split_inclusive('\n')
        .enumerate()
        .map(|(index, line)| {
            let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            CodeLine {
                anchor: format!("n{number}"),
                old_number: None,
                new_number: Some(number),
                annotation: None,
                content: html! { (line.strip_suffix('\n').unwrap_or(line)) },
                style: LineStyle::Context,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use maud::Render;

    use super::{CodeBlock, LineNumbers, text_lines};

    #[test]
    fn renders_linkable_text_lines() {
        let rendered = CodeBlock {
            summary: "source code",
            numbers: LineNumbers::Single,
            annotations: false,
            lines: text_lines("first\nsecond"),
        }
        .render()
        .into_string();

        assert!(rendered.contains("id=\"n1\""));
        assert!(rendered.contains("href=\"#n2\""));
        assert!(rendered.contains("<code>second</code>"));
    }
}
