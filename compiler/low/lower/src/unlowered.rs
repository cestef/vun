//! Why a program does not lower to LIR, and where.

use std::fmt;

use vun_mir::Unready;
use vun_origin::OriginId;
use vun_sema::ty::Ty;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Unlowered {
    pub at: OriginId,
    pub why: Why,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Why {
    /// A function reached is not concrete MIR.
    Unready(Unready),
    /// A type with no layout at run time.
    NoLayout(Ty),
    /// A form lowering does not handle yet.
    Unsupported(Form),
    /// No target is chosen.
    Untargeted,
    /// The target's addresses are as wide as no integer of LIR.
    Address(u32),
    /// A coroutine's frame would hold itself (spec 06, section 1).
    Recursive,
}

/// What lowering does not handle yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Form {
    /// A primitive called at run time that lowering has no instructions for.
    Primitive,
    /// A value converted between types whose representations differ in a way not handled.
    Conversion,
    /// A compile-time value with no run-time encoding: a type, code, a namespace.
    Constant,
    /// A call of something that is no function, closure or link symbol.
    Callee,
    /// A deferred statement consuming linear values, which the place states decide.
    Defer,
    /// A place projection on a type it does not apply to.
    Projection,
    /// Coroutine frames.
    Frame,
    /// Code values at run time.
    Quote,
}

impl fmt::Display for Unlowered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.why {
            Why::Unready(unready) => unready.fmt(f),
            Why::NoLayout(ty) => write!(f, "the type `{ty}` has no layout at run time"),
            Why::Unsupported(form) => write!(f, "lowering to LIR does not handle {} yet", form.what()),
            Why::Untargeted => f.write_str("no target is chosen"),
            Why::Address(bits) => write!(f, "addresses of {bits} bits are as wide as no integer of LIR"),
            Why::Recursive => f.write_str("the coroutine's frame would hold itself"),
        }
    }
}

impl std::error::Error for Unlowered {}

impl Form {
    #[must_use]
    pub const fn what(self) -> &'static str {
        match self {
            Self::Primitive => "this primitive at run time",
            Self::Conversion => "this conversion between types",
            Self::Constant => "this compile-time value at run time",
            Self::Callee => "calls of this kind of value",
            Self::Defer => "deferred statements consuming linear values",
            Self::Projection => "this part of this type",
            Self::Frame => "coroutine frames",
            Self::Quote => "code values at run time",
        }
    }
}

impl Unlowered {
    #[must_use]
    pub fn unsupported(at: OriginId, form: Form) -> Self {
        Self { at, why: Why::Unsupported(form) }
    }
}
