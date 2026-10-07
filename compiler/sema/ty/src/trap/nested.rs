//! The loans compile-time calls do not lend yet.

use vun_codec::{Decode, Encode};

/// Where a loan is that compile-time calls do not lend yet (decisions 135 and 516).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Nested {
    /// A loan whose target holds a loan.
    OfLoan,
    /// A loan in a closure, which never leaves the frame that made it.
    InClosure,
    /// A loan in the result of a call.
    InResult,
    /// A loan in a value an exclusive parameter is left holding.
    InParameter,
    /// A loan of the storage of the call that returns it, or of another call's.
    Own,
    /// A frame holding what it was lent.
    InFrame,
    /// A loan lent to a primitive, which changes nothing it is lent.
    ToPrimitive,
}

impl Nested {
    /// What it says.
    #[must_use]
    pub fn why(self) -> &'static str {
        match self {
            Self::OfLoan => "a loan of a place holding a loan",
            Self::InClosure => "a loan in a closure",
            Self::InResult => "a loan in the result of a call",
            Self::InParameter => "a loan in a value an exclusive parameter is left holding",
            Self::Own => "a loan of storage the call that returns it owns",
            Self::InFrame => "a frame holding what it was lent",
            Self::ToPrimitive => "a loan lent to a primitive",
        }
    }
}
