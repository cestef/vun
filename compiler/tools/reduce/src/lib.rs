//! Test case reduction (spec 13, section 10): a file that does something interesting, such as
//! crashing the compiler, is cut down to the least text that still does. Edits follow the
//! concrete syntax tree, as in hierarchical delta debugging: runs of siblings are removed, halves
//! first, then single nodes are replaced by one of their children, then single lexemes are
//! removed. A candidate that parses worse than the text it came from is dropped before it is
//! tested, and no text is tested twice. The result is laid out by the formatter when that keeps it
//! interesting.

mod edit;
mod parse;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt;

use vun_hash::Hash;
use vun_trace::tracing;

use edit::Pass;
use parse::Parse;

/// What a reduction keeps: whether a text still does what the original did.
pub trait Interest {
    fn holds(&mut self, text: &str) -> bool;
}

impl<F: FnMut(&str) -> bool> Interest for F {
    fn holds(&mut self, text: &str) -> bool {
        self(text)
    }
}

/// A reduced text and how it was reached.
#[derive(Debug)]
pub struct Reduced {
    pub text: String,
    pub stats: Stats,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Candidates the interest was asked about.
    pub tests: usize,
    /// Candidates kept.
    pub kept: usize,
    /// Bytes of the original.
    pub before: usize,
}

/// The original text is not interesting, so there is nothing to keep.
#[derive(Debug, PartialEq, Eq)]
pub struct Uninteresting;

impl fmt::Display for Uninteresting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the original is not interesting")
    }
}

impl std::error::Error for Uninteresting {}

impl Reduced {
    /// Reduces `text` while `interest` holds.
    ///
    /// # Errors
    /// Returns [`Uninteresting`] if `interest` does not hold for `text` itself.
    pub fn of(text: &str, interest: &mut dyn Interest) -> Result<Self, Uninteresting> {
        if !interest.holds(text) {
            return Err(Uninteresting);
        }
        let stats = Stats { tests: 1, kept: 0, before: text.len() };
        let tried = HashMap::from([(Hash::of(text.as_bytes()), true)]);
        let mut reducer = Reducer { text: text.to_owned(), interest, tried, stats };
        while Pass::ALL.iter().any(|&pass| reducer.exhaust(pass)) {}
        reducer.lay_out();
        tracing::info!(
            before = text.len(),
            after = reducer.text.len(),
            tests = reducer.stats.tests,
            "reduced"
        );
        Ok(Self { text: reducer.text, stats: reducer.stats })
    }
}

struct Reducer<'i> {
    text: String,
    interest: &'i mut dyn Interest,
    /// Whether each text tested, by its hash, was interesting.
    tried: HashMap<Hash, bool>,
    stats: Stats,
}

impl Reducer<'_> {
    /// Applies `pass` until no edit of it is kept, returning whether one was.
    fn exhaust(&mut self, pass: Pass) -> bool {
        let mut any = false;
        while self.step(pass) {
            any = true;
        }
        any
    }

    /// Keeps the first edit of `pass` that leaves the text shorter and interesting.
    fn step(&mut self, pass: Pass) -> bool {
        let parsed = Parse::of(&self.text);
        let edits = pass.edits(&parsed, &self.text);
        for edit in &edits {
            let candidate = edit.apply(&self.text);
            let shorter = candidate.len() < self.text.len();
            if shorter && Parse::of(&candidate).errors <= parsed.errors && self.test(&candidate) {
                tracing::debug!(?pass, size = candidate.len(), "kept");
                self.stats.kept += 1;
                self.text = candidate;
                return true;
            }
        }
        false
    }

    /// Whether `candidate` is interesting, asking the interest only about texts not tried.
    fn test(&mut self, candidate: &str) -> bool {
        let hash = Hash::of(candidate.as_bytes());
        if let Some(&holds) = self.tried.get(&hash) {
            return holds;
        }
        self.stats.tests += 1;
        let holds = self.interest.holds(candidate);
        self.tried.insert(hash, holds);
        holds
    }

    /// The text as the formatter lays it out, when it parses cleanly and stays interesting.
    fn lay_out(&mut self) {
        let parsed = Parse::of(&self.text);
        if parsed.errors > 0 {
            return;
        }
        let laid = parsed.layout(&self.text);
        if laid != self.text && self.test(&laid) {
            self.text = laid;
        }
    }
}
