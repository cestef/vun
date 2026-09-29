//! How far a production reads the tokens ahead: the forms continuing an operand are told apart by
//! the run of tokens they read before their next child.

use vun_grammar::{Automaton, Level, Positions, Read};

use super::spec::Spec;
use crate::parser::Parser;

impl Parser<'_, '_, '_> {
    /// How many tokens from the current one `automaton`, standing at `at`, reads before a child or
    /// its end, counting a child starting after them; `None` where it reads none, or cannot go on
    /// after them to a child or its end.
    pub(super) fn run(&self, automaton: &Automaton, mut at: Positions) -> Option<usize> {
        let mut ahead = 0;
        loop {
            let next = automaton.next(Some(at));
            let read = next.filter(|p| self.reads_token(&automaton.reads[p], ahead));
            let starts = |p: usize| match &automaton.reads[p] {
                Read::Child(child) => self.starts(&Spec::child(child, Level::LOWEST), ahead),
                _ => false,
            };
            if !read.is_empty() {
                at = read;
                ahead += 1;
            } else if next.iter().any(starts) {
                return Some(ahead + 1);
            } else {
                // Its leading tokens all written, a form is read though what follows is missing.
                let awaits = next.iter().any(|p| matches!(automaton.reads[p], Read::Child(_)));
                return (ahead > 0 && (awaits || automaton.accepts(Some(at)))).then_some(ahead);
            }
        }
    }
}
