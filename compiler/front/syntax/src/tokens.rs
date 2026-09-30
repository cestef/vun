//! Tokens as struct-of-arrays, with trivia in a side table.
//!
//! The parser scans only significant tokens; whitespace and comments sit in a side table, each run
//! attached to the token that follows it. Together they cover the source exactly, so the formatter
//! and the language server can rebuild it byte for byte. The last token is always `Eof`, which owns
//! any trailing trivia.

use std::fmt;

use vun_grammar::TokenKind;
use vun_index::{Idx, IndexVec, index};
use vun_span::{ByteRange, FileId, Span};

use crate::lexer::{ESCAPE, is_ident};

index! {
    pub struct TokenId;
    pub struct TriviaId;
}

/// One lexed piece of source: a token or a trivia run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lexeme {
    pub kind: TokenKind,
    pub range: ByteRange,
}

pub struct Tokens {
    file: FileId,
    kinds: IndexVec<TokenId, TokenKind>,
    ranges: IndexVec<TokenId, ByteRange>,
    /// The first trivia before each token; a token's trivia ends where the next token's begins.
    leading: IndexVec<TokenId, TriviaId>,
    trivia: IndexVec<TriviaId, Lexeme>,
    /// Tokens the lexer reported as malformed, in order. Later stages stay quiet about them.
    diagnosed: Vec<TokenId>,
}

impl Tokens {
    /// The source text naming `name`: itself, after `\` when it is a keyword; `None` when it does
    /// not lex as a name.
    #[must_use]
    pub fn spell_name(name: &str) -> Option<String> {
        let escape = TokenKind::keyword(name).map(|_| ESCAPE);
        is_ident(name).then(|| escape.into_iter().chain(name.chars()).collect())
    }

    #[must_use]
    pub fn file(&self) -> FileId {
        self.file
    }

    /// Number of significant tokens, including the final `Eof`.
    #[must_use]
    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    #[must_use]
    pub fn kind(&self, t: TokenId) -> TokenKind {
        self.kinds[t]
    }

    #[must_use]
    pub fn range(&self, t: TokenId) -> ByteRange {
        self.ranges[t]
    }

    #[must_use]
    pub fn span(&self, t: TokenId) -> Span {
        Span { file: self.file, range: self.ranges[t] }
    }

    pub fn ids(&self) -> impl Iterator<Item = TokenId> {
        (0..self.len()).map(TokenId::new)
    }

    /// Whether the lexer reported `t` as malformed.
    #[must_use]
    pub fn diagnosed(&self, t: TokenId) -> bool {
        self.diagnosed.binary_search(&t).is_ok()
    }

    /// The trivia immediately before `t`.
    pub fn leading_trivia(&self, t: TokenId) -> impl Iterator<Item = Lexeme> + '_ {
        let start = self.leading[t].index();
        let next = self.leading.get(TokenId::new(t.index() + 1));
        let end = next.map_or(self.trivia.len(), |id| id.index());
        (start..end).map(|i| self.trivia[TriviaId::new(i)])
    }

    /// The text of the doc comment `t` of `source` (`///` or `//!`), without its marker and the
    /// space after it; `None` when `t` is not a doc comment.
    #[must_use]
    pub fn doc_text<'s>(&self, t: TokenId, source: &'s str) -> Option<&'s str> {
        let marker = match self.kinds[t] {
            TokenKind::DocComment => crate::lexer::DOC_COMMENT,
            TokenKind::ModuleDocComment => crate::lexer::MODULE_DOC_COMMENT,
            _ => return None,
        };
        let line = self.ranges[t].of(source).strip_prefix(marker)?;
        Some(line.strip_prefix(' ').unwrap_or(line))
    }

    /// Every token and trivia run, in source order.
    pub fn lexemes(&self) -> impl Iterator<Item = Lexeme> + '_ {
        self.ids().flat_map(move |t| {
            let token = Lexeme { kind: self.kinds[t], range: self.ranges[t] };
            self.leading_trivia(t).chain(std::iter::once(token))
        })
    }

    /// A line per lexeme, for `vun debug dump tokens` and snapshots.
    #[must_use]
    pub fn dump<'a>(&'a self, source: &'a str) -> Dump<'a> {
        Dump { tokens: self, source }
    }
}

/// Why a token stream does not cover its source exactly; see [`Tokens::verify`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uncovered {
    /// A lexeme does not start where the previous one ended.
    Gap { expected: u32, found: ByteRange },
    /// The lexemes end before the source does.
    Short { end: u32, source_len: usize },
    /// The stream does not end with `Eof`.
    MissingEof,
}

impl Tokens {
    /// Checks the lexer's invariant: the lexemes cover `source` exactly, in order, and end with
    /// `Eof`. Tests and fuzz targets call this; it is the verifier of this representation.
    ///
    /// # Errors
    /// Returns the first violation found.
    pub fn verify(&self, source: &str) -> Result<(), Uncovered> {
        let mut end = 0;
        for lexeme in self.lexemes() {
            if lexeme.range.lo != end {
                return Err(Uncovered::Gap { expected: end, found: lexeme.range });
            }
            end = lexeme.range.hi;
        }
        if end as usize != source.len() {
            return Err(Uncovered::Short { end, source_len: source.len() });
        }
        match self.kinds.last() {
            Some(TokenKind::Eof) => Ok(()),
            _ => Err(Uncovered::MissingEof),
        }
    }
}

/// See [`Tokens::dump`].
pub struct Dump<'a> {
    tokens: &'a Tokens,
    source: &'a str,
}

impl fmt::Display for Dump<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for lexeme in self.tokens.lexemes() {
            let ByteRange { lo, hi } = lexeme.range;
            writeln!(f, "{:?} {lo}..{hi} {:?}", lexeme.kind, lexeme.range.of(self.source))?;
        }
        Ok(())
    }
}

/// Accumulates lexemes in source order.
pub(crate) struct Builder {
    tokens: Tokens,
    unclaimed: TriviaId,
}

impl Builder {
    pub(crate) fn new(file: FileId, capacity: usize) -> Self {
        let tokens = Tokens {
            file,
            kinds: IndexVec::with_capacity(capacity),
            ranges: IndexVec::with_capacity(capacity),
            leading: IndexVec::with_capacity(capacity),
            trivia: IndexVec::new(),
            diagnosed: Vec::new(),
        };
        Self { tokens, unclaimed: TriviaId::new(0) }
    }

    pub(crate) fn push(&mut self, lexeme: Lexeme) {
        let t = &mut self.tokens;
        if lexeme.kind.is_trivia() {
            t.trivia.push(lexeme);
            return;
        }
        t.kinds.push(lexeme.kind);
        t.ranges.push(lexeme.range);
        t.leading.push(self.unclaimed);
        self.unclaimed = TriviaId::new(t.trivia.len());
    }

    /// Records that the last token pushed was reported as malformed.
    pub(crate) fn diagnosed(&mut self) {
        let last = TokenId::new(self.tokens.kinds.len() - 1);
        self.tokens.diagnosed.push(last);
    }

    /// Ends the stream with `Eof` at `end`, which claims the trailing trivia.
    pub(crate) fn finish(mut self, end: u32) -> Tokens {
        self.push(Lexeme { kind: TokenKind::Eof, range: ByteRange::new(end, end) });
        self.tokens
    }
}
