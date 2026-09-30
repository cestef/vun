//! Why an instantiation's body is not ready for code generation, and where it stopped.

use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Msg};
use vun_hir::db::Parsed;
use vun_origin::OriginId;
use vun_query::Cx;
use vun_sema::typeck::{Leaves, Typed};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::trap::Unevaluated;

use crate::{Unbuilt, Why};

/// Why an instantiation's body is not ready for code generation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unready {
    /// A form MIR does not lower yet.
    Unbuilt(Unbuilt),
    /// A compile-time value the body reads has none.
    Unevaluated { at: OriginId, why: Unevaluated },
    /// A local typing found no type for, or a form MIR stopped at holding a form typing gave up
    /// on, and what typing says of it, if it says: the error it reported, which in a library,
    /// whose bodies are reported only as a program reaches them, is what to show, or else the
    /// form it gave up on.
    Untyped { at: OriginId, said: Option<Box<Diagnostic>> },
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
            Self::Untyped { said: Some(said), .. } => {
                write!(f, "typing found no type for this: {}", said.message)
            }
            Self::Untyped { said: None, .. } => {
                f.write_str("typing found no type for this and reported no error: a compiler bug")
            }
        }
    }
}

impl Unready {
    /// This, or what typing says of the body where it stopped: the form typing gave up on under
    /// that place, which a form MIR does not build or a value it has no type for follows from,
    /// or, for a local left untyped, the error typing reported.
    pub(super) fn explained(self, cx: &Cx<'_>, i: &Instantiation) -> Self {
        let Some(typed) = cx.get::<Typed>(i) else { return self };
        let at = self.at();
        let reported = match self {
            Self::Untyped { .. } => typed.diag.first_error().cloned(),
            Self::Unbuilt(_) | Self::Unevaluated { .. } => None,
        };
        let gave_up = || {
            let d = typed.gave_up_under(&cx.get::<Parsed>(&i.f.literal.file)?.hir, at, Leaves::Nothing)?;
            let within = d.primary.as_ref().is_some_and(|label| label.origin != at);
            let msg = Msg::new().text("lowering stops at this, which needs its type");
            Some(if within { d.label(at, Some(msg)) } else { d })
        };
        match reported.or_else(gave_up) {
            Some(said) => Self::Untyped { at, said: Some(Box::new(said)) },
            None => self,
        }
    }

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
