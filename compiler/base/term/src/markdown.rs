//! Markdown pages, such as diagnostic explanations, styled for a terminal: headings, code
//! blocks and inline code. The text stays as written; only styles are added.

use crate::{Style, Text};

/// Opens and closes a code block.
const FENCE: &str = "```";
/// Starts a heading.
const HEADING: char = '#';
/// Delimits inline code.
const CODE: char = '`';
/// Starts a command in a transcript.
const PROMPT: &str = "$ ";

impl Text {
    /// `source` as Markdown: headings as headings, code blocks dimmed but for the commands of
    /// transcripts, inline code as literals.
    #[must_use]
    pub fn markdown(source: &str) -> Self {
        let mut text = Self::new();
        let mut in_block = false;
        for line in source.lines() {
            text = if line.starts_with(FENCE) {
                in_block = !in_block;
                text.styled(Style::Dim, line.to_owned())
            } else if in_block && line.starts_with(PROMPT) {
                text.styled(Style::Literal, line.to_owned())
            } else if in_block {
                text.plain(line.to_owned())
            } else if line.starts_with(HEADING) {
                text.styled(Style::Heading, line.to_owned())
            } else {
                text.append(Self::inline(line))
            };
            text = text.newline();
        }
        text
    }

    /// A line of prose, its inline code as literals.
    fn inline(line: &str) -> Self {
        line.split(CODE).enumerate().fold(Self::new(), |text, (i, part)| {
            let code = i % 2 == 1;
            let text = if code { text.plain(CODE.to_string()) } else { text };
            let text = text.maybe(code.then_some(Style::Literal), part.to_owned());
            if code { text.plain(CODE.to_string()) } else { text }
        })
    }
}
