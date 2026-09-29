//! Writing a laid-out file: its tokens and comments in order, each after the whitespace its place
//! asks for (spec 13, section 5). Line breaks are the author's; the rest is computed.

use std::fmt;

use vun_grammar::{NodeKind, SEPARATORS, TokenKind};
use vun_syntax::lexer::TO_LINE_END;
use vun_syntax::literal::INDENT;
use vun_syntax::tokens::{TokenId, Tokens};

use crate::Layout;
use crate::spacing::{Between, Group, Tok};

/// A group still open, and the indent of the line holding its opener.
struct Open {
    group: Group,
    indent: usize,
}

/// The last token written: what spacing reads of it, and what the group it closed, if it closed
/// one, was opened by.
struct Last {
    tok: Tok,
    closed: Option<NodeKind>,
}

pub(crate) struct Writer<'l, 'a, 'f, 'o> {
    layout: &'l Layout<'a>,
    out: &'f mut fmt::Formatter<'o>,
    groups: Vec<Open>,
    /// The indent of the line being written.
    indent: usize,
    last: Option<Last>,
    /// What the line being written holds so far.
    line: String,
    /// Whether anything is written yet.
    started: bool,
}

impl<'l, 'a, 'f, 'o> Writer<'l, 'a, 'f, 'o> {
    pub(crate) fn new(layout: &'l Layout<'a>, out: &'f mut fmt::Formatter<'o>) -> Self {
        Self { layout, out, groups: Vec::new(), indent: 0, last: None, line: String::new(), started: false }
    }

    /// Writes the whole file, ending it with one newline.
    pub(crate) fn file(mut self) -> fmt::Result {
        let tokens = self.layout.tokens;
        for t in tokens.ids() {
            // The line breaks of the whitespace before each comment and before the token.
            let mut breaks = 0;
            for lexeme in tokens.leading_trivia(t) {
                let text = lexeme.range.of(self.layout.source);
                match lexeme.kind {
                    TokenKind::Comment => {
                        self.comment(text.trim_end(), breaks, t)?;
                        breaks = 0;
                    }
                    _ => breaks += text.matches('\n').count(),
                }
            }
            if tokens.kind(t) == TokenKind::Eof {
                break;
            }
            self.token(t, breaks)?;
        }
        if self.started { self.out.write_str("\n") } else { Ok(()) }
    }

    /// The comment `text` before the token `next`, after `breaks` line breaks.
    fn comment(&mut self, text: &str, breaks: usize, next: TokenId) -> fmt::Result {
        if self.started && breaks == 0 {
            self.put(" ")?;
        } else {
            // A comment before a closer is inside its group.
            let closes = self.closing(next).is_some();
            let indent =
                if closes { self.groups.last().map_or(0, |g| g.indent + 1) } else { self.line_indent(next) };
            self.line(breaks, indent, closes)?;
        }
        self.put(text)?;
        self.started = true;
        Ok(())
    }

    /// Writes `text` on the line being written.
    fn put(&mut self, text: &str) -> fmt::Result {
        self.line.push_str(text);
        self.out.write_str(text)
    }

    /// The token `t`, after `breaks` line breaks.
    fn token(&mut self, t: TokenId, breaks: usize) -> fmt::Result {
        let tokens = self.layout.tokens;
        let roles = &self.layout.roles;
        let tok = Tok { kind: tokens.kind(t), anchor: roles.anchor(t), open: roles.is_open(t) };
        let closing = self.closing(t);
        // A token running to the end of its line ends before the newline written after it: a
        // carriage return it ends with would become part of that line end.
        let text = tokens.range(t).of(self.layout.source);
        let text = if TO_LINE_END.contains(&tok.kind) { text.trim_end_matches('\r') } else { text };
        if breaks > 0 && self.started {
            let indent = self.line_indent(t);
            self.line(breaks, indent, closing.is_some())?;
        } else if let Some(last) = &self.last {
            let between = Between { closing, closed: last.closed };
            // Tokens that would lex as others together stay apart: `2 .5`, `.. =`.
            if last.tok.spaced(tok, &between) || !Tokens::apart(tokens.file(), &self.line, text) {
                self.put(" ")?;
            }
        }
        self.put(text)?;
        self.started = true;
        let closed = if closing.is_some() { self.groups.pop().and_then(|g| g.group.anchor) } else { None };
        if let Some(row) = tok.kind.delimiter() {
            self.groups.push(Open { group: Group { row, anchor: tok.anchor }, indent: self.indent });
        }
        self.last = Some(Last { tok, closed });
        Ok(())
    }

    /// Starts a new line at `indent`, after a blank one when the source had one, unless it
    /// follows an opening delimiter or `closes` one.
    fn line(&mut self, breaks: usize, indent: usize, closes: bool) -> fmt::Result {
        if !self.started {
            return Ok(());
        }
        let opened = self.last.as_ref().is_some_and(|l| l.tok.kind.opens());
        self.out.write_str(if breaks > 1 && !opened && !closes { "\n\n" } else { "\n" })?;
        self.line.clear();
        for _ in 0..indent {
            self.put(INDENT)?;
        }
        self.indent = indent;
        Ok(())
    }

    /// The group `t` closes, if it closes the innermost one open.
    fn closing(&self, t: TokenId) -> Option<Group> {
        let kind = self.layout.tokens.kind(t);
        self.groups.last().map(|g| g.group).filter(|g| g.row.close == kind)
    }

    /// The indent of a line starting with `t`: its group's closer is at the opener's line, an
    /// entry one level inside its group, a line continuing one a level more, and a raw string
    /// line after another at its indent.
    fn line_indent(&self, t: TokenId) -> usize {
        let kind = self.layout.tokens.kind(t);
        let top = self.groups.last();
        if let Some(open) = top.filter(|g| g.group.row.close == kind) {
            return open.indent;
        }
        let last = self.last.as_ref().map(|l| l.tok.kind);
        if kind == TokenKind::RawStringLine && last == Some(TokenKind::RawStringLine) {
            return self.indent;
        }
        let base = top.map_or(0, |g| g.indent + 1);
        // A separator starting a line ends its entry there: the `;` after a raw string.
        let entry = self.layout.roles.starts_entry(t)
            || last.is_none_or(TokenKind::opens)
            || SEPARATORS.contains(&kind)
            || TokenKind::END.contains(&kind);
        if entry { base } else { base + 1 }
    }
}
