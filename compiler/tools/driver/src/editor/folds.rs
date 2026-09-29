//! What an editor may fold in a file: each pair of delimiters, each run of raw string lines and
//! each run of comments, when it spans several lines.

use vun_grammar::TokenKind;
use vun_span::{ByteRange, FileId};

use crate::Session;

/// A part of a file that folds, from its first byte to its last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fold {
    pub range: ByteRange,
    pub kind: FoldKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldKind {
    /// A group or a string.
    Region,
    Comment,
}

/// A run of lexemes of one kind of fold, as it grows.
#[derive(Default)]
struct Runs {
    open: Vec<ByteRange>,
    run: Option<(ByteRange, FoldKind)>,
    folds: Vec<Fold>,
}

impl Runs {
    /// Extends the run of `kind` by `range`, ending the run of another kind first.
    fn extend(&mut self, kind: FoldKind, range: ByteRange, text: &str) {
        match self.run {
            Some((run, k)) if k == kind => self.run = Some((run.cover(range), kind)),
            _ => {
                self.end(text);
                self.run = Some((range, kind));
            }
        }
    }

    fn end(&mut self, text: &str) {
        if let Some((range, kind)) = self.run.take() {
            self.push(range, kind, text);
        }
    }

    fn push(&mut self, range: ByteRange, kind: FoldKind, text: &str) {
        if range.of(text).contains('\n') {
            self.folds.push(Fold { range, kind });
        }
    }
}

impl Session {
    /// The folds of `file`, by where they start; those within one line are left out.
    pub fn folds(&mut self, file: FileId) -> Vec<Fold> {
        let parsed = self.syntax(file);
        let text = self.text(file);
        let mut runs = Runs::default();
        for lexeme in parsed.tokens.lexemes() {
            let kind = lexeme.kind;
            match kind {
                TokenKind::Comment | TokenKind::DocComment | TokenKind::ModuleDocComment => {
                    runs.extend(FoldKind::Comment, lexeme.range, text);
                }
                TokenKind::RawStringLine => runs.extend(FoldKind::Region, lexeme.range, text),
                // A line break alone keeps a run going; a blank line ends it.
                TokenKind::Whitespace if lexeme.range.of(text).matches('\n').count() < 2 => {}
                _ => runs.end(text),
            }
            if kind.opens() {
                runs.open.push(lexeme.range);
            } else if kind.closes()
                && let Some(open) = runs.open.pop()
            {
                runs.push(open.cover(lexeme.range), FoldKind::Region, text);
            }
        }
        runs.end(text);
        runs.folds.sort_by_key(|f| f.range.lo);
        runs.folds
    }
}
