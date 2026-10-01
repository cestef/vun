//! Writing a laid-out file: its tokens and comments in order, each after the whitespace its place
//! asks for (spec 13, section 5). Line breaks are the author's, and those `forced` adds; the rest
//! is computed. A pass also traces where each token and each group landed, which says what to
//! break next (the module `wrap`).

use std::collections::BTreeSet;

use vun_grammar::{NodeKind, SEPARATORS, TokenKind};
use vun_syntax::lexer::TO_LINE_END;
use vun_syntax::literal::INDENT;
use vun_syntax::tokens::{TokenId, Tokens};

use crate::Layout;
use crate::spacing::{Between, Group, Tok};

/// A group still open, the indent of the line holding its opener, and its place in the trace.
struct Open {
    group: Group,
    indent: usize,
    traced: usize,
}

/// Where a token landed: the line of the output, and the column its text ends at.
pub(crate) struct Placed {
    pub(crate) token: TokenId,
    pub(crate) line: usize,
    pub(crate) end: usize,
}

/// A delimited group as written: its opener and closer, and the tokens starting its entries.
pub(crate) struct Written {
    pub(crate) open: TokenId,
    pub(crate) close: Option<TokenId>,
    pub(crate) entries: Vec<TokenId>,
}

/// What a pass wrote, token by token and group by group, in order.
#[derive(Default)]
pub(crate) struct Trace {
    pub(crate) placed: Vec<Placed>,
    pub(crate) groups: Vec<Written>,
    /// The infix operators with a right operand, and how many groups were open where each stood:
    /// where a long line may be continued.
    pub(crate) infix: Vec<(TokenId, usize)>,
}

/// The last token written: what spacing reads of it, and what the group it closed, if it closed
/// one, was opened by.
struct Last {
    tok: Tok,
    closed: Option<NodeKind>,
}

pub(crate) struct Writer<'l, 'a, 'f> {
    layout: &'l Layout<'a>,
    out: &'f mut String,
    /// The tokens a line break is added before, whatever the author wrote.
    forced: &'f BTreeSet<TokenId>,
    trace: Trace,
    /// The line of the output being written, from 0.
    at: usize,
    groups: Vec<Open>,
    /// The indent of the line being written.
    indent: usize,
    last: Option<Last>,
    /// What the line being written holds so far.
    line: String,
    /// Whether anything is written yet.
    started: bool,
}

impl<'l, 'a, 'f> Writer<'l, 'a, 'f> {
    pub(crate) fn new(layout: &'l Layout<'a>, out: &'f mut String, forced: &'f BTreeSet<TokenId>) -> Self {
        Self {
            layout,
            out,
            forced,
            trace: Trace::default(),
            at: 0,
            groups: Vec::new(),
            indent: 0,
            last: None,
            line: String::new(),
            started: false,
        }
    }

    /// Writes the whole file, ending it with one newline, and gives what it traced.
    pub(crate) fn file(mut self) -> Trace {
        let tokens = self.layout.tokens;
        for t in tokens.ids() {
            // The line breaks of the whitespace before each comment and before the token.
            let mut breaks = 0;
            for lexeme in tokens.leading_trivia(t) {
                let text = lexeme.range.of(self.layout.source);
                match lexeme.kind {
                    TokenKind::Comment => {
                        self.comment(text.trim_end(), breaks, t);
                        breaks = 0;
                    }
                    _ => breaks += text.matches('\n').count(),
                }
            }
            if tokens.kind(t) == TokenKind::Eof {
                break;
            }
            if self.forced.contains(&t) {
                breaks = breaks.max(1);
            }
            self.token(t, breaks);
        }
        if self.started {
            self.out.push('\n');
        }
        self.trace
    }

    /// The comment `text` before the token `next`, after `breaks` line breaks.
    fn comment(&mut self, text: &str, breaks: usize, next: TokenId) {
        if self.started && breaks == 0 {
            self.put(" ");
        } else {
            // A comment before a closer is inside its group.
            let closes = self.closing(next).is_some();
            let indent =
                if closes { self.groups.last().map_or(0, |g| g.indent + 1) } else { self.line_indent(next) };
            self.line(breaks, indent, closes);
        }
        self.put(text);
        self.started = true;
    }

    /// Writes `text` on the line being written.
    fn put(&mut self, text: &str) {
        self.line.push_str(text);
        self.out.push_str(text);
    }

    /// The token `t`, after `breaks` line breaks.
    fn token(&mut self, t: TokenId, breaks: usize) {
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
            self.line(breaks, indent, closing.is_some());
        } else if let Some(last) = &self.last {
            let between = Between { closing, closed: last.closed };
            // Tokens that would lex as others together stay apart: `2 .5`, `.. =`.
            if last.tok.spaced(tok, &between) || !Tokens::apart(tokens.file(), &self.line, text) {
                self.put(" ");
            }
        }
        // A token starting an entry of the group it is in is where that group may be broken.
        if roles.starts_entry(t)
            && closing.is_none()
            && let Some(open) = self.groups.last()
        {
            self.trace.groups[open.traced].entries.push(t);
        }
        if tok.anchor == Some(NodeKind::Infix) && !tok.open {
            self.trace.infix.push((t, self.groups.len()));
        }
        self.put(text);
        self.trace.placed.push(Placed { token: t, line: self.at, end: self.line.chars().count() });
        self.started = true;
        let closed = match closing {
            Some(_) => self.groups.pop().and_then(|g| {
                self.trace.groups[g.traced].close = Some(t);
                g.group.anchor
            }),
            None => None,
        };
        if let Some(row) = tok.kind.delimiter() {
            let traced = self.trace.groups.len();
            self.trace.groups.push(Written { open: t, close: None, entries: Vec::new() });
            self.groups.push(Open { group: Group { row, anchor: tok.anchor }, indent: self.indent, traced });
        }
        self.last = Some(Last { tok, closed });
    }

    /// Starts a new line at `indent`, after a blank one when the source had one, unless it
    /// follows an opening delimiter or `closes` one.
    fn line(&mut self, breaks: usize, indent: usize, closes: bool) {
        if !self.started {
            return;
        }
        let opened = self.last.as_ref().is_some_and(|l| l.tok.kind.opens());
        let blank = breaks > 1 && !opened && !closes;
        self.out.push_str(if blank { "\n\n" } else { "\n" });
        self.at += if blank { 2 } else { 1 };
        self.line.clear();
        for _ in 0..indent {
            self.put(INDENT);
        }
        self.indent = indent;
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
