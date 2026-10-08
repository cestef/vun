//! What the model says a program prints, against what the backends agreed it printed.

use std::fmt;

use vun_smith::eval::{Machine, Trap};
use vun_smith::ir::Program;

/// How a program that every backend ran alike printed other than the model says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Divergence {
    /// The model does not run the program to its end: it made one that traps or never ends.
    Model(Trap),
    /// The line at `line` (from 0) is `printed` where the model has `expected`; each `None` where
    /// there is no such line, or it is no number.
    Output { line: usize, expected: Option<u64>, printed: Option<u64> },
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(trap) => write!(f, "the model does not run it: {trap}"),
            Self::Output { line, expected, printed } => {
                write!(f, "line {line} of the output is {printed:?}, the model has {expected:?}")
            }
        }
    }
}

impl std::error::Error for Divergence {}

impl Divergence {
    /// How `stdout`, the output of `program`, differs from what the model computes, if it does.
    #[must_use]
    pub fn between(program: &Program, fuel: u64, stdout: &[u8]) -> Option<Self> {
        let expected = match Machine::new(program, fuel).hashes() {
            Ok(hashes) => hashes,
            Err(trap) => return Some(Self::Model(trap)),
        };
        let printed: Vec<Option<u64>> =
            String::from_utf8_lossy(stdout).lines().map(|l| l.trim().parse().ok()).collect();
        (0..expected.len().max(printed.len())).find_map(|line| {
            let (expected, printed) = (expected.get(line).copied(), printed.get(line).copied().flatten());
            (expected != printed).then_some(Self::Output { line, expected, printed })
        })
    }
}
