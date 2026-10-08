//! Making a program that found something smaller while it still finds it.

use vun_driver::trial::Cause;

use crate::model::Divergence;
use crate::{Campaign, Tried};

/// What a program found, apart from where in it: what a reduced program must still find.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The compiler against itself.
    Cause(Cause),
    /// The compiler against the model: the model could not run it, or the output differs.
    Model,
    Output,
}

impl Tried {
    /// What the program found, if anything.
    #[must_use]
    pub fn outcome(&self) -> Option<Outcome> {
        match (&self.result, &self.divergence) {
            (Err(finding), _) => Some(Outcome::Cause(finding.cause())),
            (Ok(_), Some(Divergence::Model(_))) => Some(Outcome::Model),
            (Ok(_), Some(Divergence::Output { .. })) => Some(Outcome::Output),
            (Ok(_), None) => None,
        }
    }
}

impl Campaign<'_> {
    /// The smallest program found from `tried` by the ways of making it smaller that keep its
    /// outcome, after at most `trials` tries.
    pub fn reduced(&mut self, tried: Tried, trials: usize) -> Tried {
        let Some(outcome) = tried.outcome() else { return tried };
        let mut best = tried;
        let mut spent = 0;
        loop {
            let mut progressed = false;
            let mut nth = 0;
            while nth < best.program.shrinks() {
                let Some(smaller) = best.program.shrunk(nth) else { break };
                if spent >= trials {
                    return best;
                }
                spent += 1;
                let tried = self.tried(best.seed, best.spelling, smaller);
                if tried.outcome().as_ref() == Some(&outcome) {
                    best = tried;
                    progressed = true;
                } else {
                    nth += 1;
                }
            }
            if !progressed {
                return best;
            }
        }
    }
}
