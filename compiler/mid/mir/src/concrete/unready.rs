//! Why an instantiation's body is not ready for code generation, and where it stopped.

use vun_codec::{Decode, Encode};
use vun_diag::Diagnostic;
use vun_origin::OriginId;
use vun_sema_ty::trap::Unevaluated;

use crate::{Unbuilt, Why};

/// Why an instantiation's body is not ready for code generation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unready {
    /// A form MIR does not lower yet.
    Unbuilt(Unbuilt),
    /// A compile-time value the body reads has none.
    Unevaluated { at: OriginId, why: Unevaluated },
    /// A local typing found no type for, and the error typing reported, if it did: in a library,
    /// whose bodies are reported only as a program reaches them, it is what to show.
    Untyped { at: OriginId, reported: Option<Box<Diagnostic>> },
}

impl From<Unbuilt> for Unready {
    fn from(unbuilt: Unbuilt) -> Self {
        Self::Unbuilt(unbuilt)
    }
}

impl std::fmt::Display for Unready {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unbuilt(Unbuilt { why: Why::Form, .. }) => {
                f.write_str("this form is not supported in compiled code yet")
            }
            Self::Unbuilt(Unbuilt { why: Why::Unevaluated(why), .. }) | Self::Unevaluated { why, .. } => {
                write!(f, "a value this needs at compile time has none: {}", why.why())
            }
            Self::Untyped { .. } => {
                f.write_str("typing found no type for this and reported no error: a compiler bug")
            }
        }
    }
}

impl Unready {
    /// Where the body stopped.
    #[must_use]
    pub fn at(&self) -> OriginId {
        match self {
            Self::Unbuilt(Unbuilt { at, .. }) | Self::Unevaluated { at, .. } | Self::Untyped { at, .. } => {
                *at
            }
        }
    }
}
