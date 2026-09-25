//! Compile-time traps (spec 03, section 7): what fired, where, and the calls it fired in. A trap
//! is reported once, at the evaluation root that needed the value, with the trap site labeled
//! and a "while evaluating" frame for each call between them.

use vun_diag::{Diagnostic, Msg, TraceFrame, errors};
use vun_origin::OriginId;

use crate::env::Unevaluated;

/// Why a trap fired.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    /// An argument of a compile-time call that is not a value of its parameter's type, such as
    /// a negative length for `lang.mem.Array`.
    Argument,
    /// A frame asked of a function that does not suspend (`lang.meta.Frame`).
    NoFrame,
    /// A trap site of the program was reached: `lang.panic.trap`.
    Trap,
    /// `lang.panic.fail` was reached, for its `lang.panic.Reason`.
    Failed(Failure),
    /// Library code reported an error (`lang.diag.error`): its message, and where it points
    /// when it names a place.
    Reported {
        message: String,
        at: Option<OriginId>,
    },
}

/// What a reached `lang.panic.fail` says: a variant of `lang.panic.Reason`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Failure {
    Message(String),
    /// A condition that did not hold: its source, and where it is written.
    Assertion {
        source: String,
        at: OriginId,
    },
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
    fn msg(&self) -> Msg {
        Msg::new().text(match self {
            Self::DivisionByZero => "division by zero",
            Self::Shift => "shift past any width",
            Self::Narrowing => "value outside the type it is narrowed to",
            Self::Budget => "the call ran out of its step budget",
            Self::Bounds => "index outside the items",
            Self::Argument => "argument outside the type of its parameter",
            Self::Trap => "a trap site was reached",
            Self::Failed(Failure::Assertion { source, .. }) => {
                return Msg::new().text("assertion failed: ").code(source.clone());
            }
            Self::NoFrame => "the function does not suspend, so it makes no frame",
            Self::Reported { message, .. } | Self::Failed(Failure::Message(message)) => {
                return Msg::new().text(message.clone());
            }
        })
    }

    /// Where the reason itself points, when it names a place: the report is shown there, not
    /// where the library code reached it.
    fn place(&self) -> Option<OriginId> {
        match self {
            Self::Reported { at, .. } => *at,
            Self::Failed(Failure::Assertion { at, .. }) => Some(*at),
            _ => None,
        }
    }
}

impl Unevaluated {
    /// Why evaluation stopped, as a report says it after "could not run: ".
    #[must_use]
    pub fn why(&self) -> &'static str {
        match self {
            Self::Runtime => "it reads a value known only at run time",
            Self::Interpreter => "no evaluator is installed",
            Self::Cycle => "it depends on itself",
            Self::Absent => "a name leads nowhere",
            Self::Provided => "it reads a declaration provided from outside",
            Self::Trapped(_) | Self::Failed => "a declaration it reads failed to evaluate",
            Self::Unsupported => "it uses a form compile-time evaluation does not run yet",
            Self::Untargeted => "it depends on the target, which is not chosen",
        }
    }

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
        // A library's report is its own message; a trap says evaluation failed, and why.
        let (id, failed) = match &self.reason {
            Reason::Reported { .. } => (errors::COMPTIME_REPORTED, false),
            Reason::Budget => (errors::COMPTIME_EXHAUSTED, true),
            _ => (errors::COMPTIME_TRAP, true),
        };
        let msg = if failed { Msg::new().text("compile-time evaluation failed: ") } else { Msg::new() };
        // A report naming a place is shown there, the value that needed it labeled.
        let mut d = match self.reason.place() {
            Some(at) => Diagnostic::error(id, msg.append(self.reason.msg()))
                .at(at, None)
                .label(root, Some(Msg::new().text("while computing this"))),
            None => Diagnostic::error(id, msg.append(self.reason.msg())).at(root, None),
        };
        // Where a library's report or a failed assertion fires is always the library's call of
        // the primitive: not shown.
        let shown = !matches!(self.reason, Reason::Reported { .. }) && self.reason.place().is_none();
        if let Some(at) = self.at.filter(|&at| at != root && shown) {
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
