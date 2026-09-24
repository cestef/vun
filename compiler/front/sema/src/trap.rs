//! Compile-time traps (spec 03, section 7): what fired, where, and the calls it fired in. A trap
//! is reported once, at the evaluation root that needed the value, with the trap site labeled
//! and a "while evaluating" frame for each call between them.

use vun_diag::{Diagnostic, Msg, TraceFrame, errors};
use vun_origin::OriginId;

use crate::env::Unevaluated;

/// Why a trap fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    DivisionByZero,
    /// A shift by more than any width can hold.
    Shift,
    /// A value outside the type it is narrowed to with `.to` (spec 04, rule 3).
    Narrowing,
    /// The call ran out of its step budget.
    Budget,
    /// An index outside the items indexed.
    Bounds,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Trap {
    pub reason: Reason,
    /// Where it fired; `None` until the evaluator attaches the statement.
    pub at: Option<OriginId>,
    /// The call sites it fired in, innermost first.
    pub calls: Vec<OriginId>,
}

impl Reason {
    fn msg(self) -> Msg {
        Msg::new().text(match self {
            Self::DivisionByZero => "division by zero",
            Self::Shift => "shift past any width",
            Self::Narrowing => "value outside the type it is narrowed to",
            Self::Budget => "the call ran out of its step budget",
            Self::Bounds => "index outside the items",
        })
    }
}

impl Unevaluated {
    /// A trap for `reason` that has not been placed yet.
    #[must_use]
    pub fn trap(reason: Reason) -> Self {
        Self::Trapped(Box::new(Trap { reason, at: None, calls: Vec::new() }))
    }

    /// This failure as seen from the statement or call at `origin`: an unplaced trap fired there,
    /// and a placed one fired in the call made there.
    #[must_use]
    pub fn through(self, origin: OriginId) -> Self {
        match self {
            Self::Trapped(mut trap) => {
                match trap.at {
                    None => trap.at = Some(origin),
                    Some(_) => trap.calls.push(origin),
                }
                Self::Trapped(trap)
            }
            other => other,
        }
    }
}

impl Trap {
    /// The report of this trap, fired while evaluating the expression at `root`.
    #[must_use]
    pub fn report(&self, root: OriginId) -> Diagnostic {
        let id = match self.reason {
            Reason::Budget => errors::COMPTIME_EXHAUSTED,
            _ => errors::COMPTIME_TRAP,
        };
        let msg = Msg::new().text("compile-time evaluation failed: ").append(self.reason.msg());
        let mut d = Diagnostic::error(id, msg).at(root, None);
        if let Some(at) = self.at.filter(|&at| at != root) {
            d = d.label(at, Some(Msg::new().text("it fires here")));
        }
        d.trace = self
            .calls
            .iter()
            .filter(|&&origin| origin != root)
            .map(|&origin| TraceFrame { msg: "while evaluating this call".into(), origin })
            .collect();
        d
    }
}
