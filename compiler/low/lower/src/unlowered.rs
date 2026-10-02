//! Why a program does not lower to LIR, and where.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Msg, TraceFrame, errors};
use vun_inline::Needed;
use vun_mir::Unready;
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_sema_ty::Ty;
use vun_sema_ty::trap::{Reason, Trap, Unevaluated};
use vun_target::Convention;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Unlowered {
    pub at: OriginId,
    pub why: Why,
    /// The calls that led to it from outside generic code, innermost first (`Program::leading`).
    pub calls: Vec<OriginId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Why {
    /// A function reached is not concrete MIR.
    Unready(Unready),
    /// A type with no layout at run time.
    NoLayout(Ty),
    /// A form lowering does not handle yet.
    Unsupported(Form),
    /// A primitive lowering has no instructions for at run time yet.
    Primitive(Primitive),
    /// No target is chosen.
    Untargeted,
    /// The target's addresses are as wide as no integer of LIR.
    Address(u32),
    /// A coroutine's frame would hold itself (spec 06, section 1).
    Recursive,
    /// An instruction of a block needs this of the target's processor, which lacks it.
    Unavailable(Needed),
    /// Library code reached at run time reports an error (`lang.diag.error`): the program is
    /// refused (spec 10, section 3).
    Reported(Reason),
    /// A function marked with a calling convention of another architecture.
    Convention(Convention),
    /// An address made into a value of a type that is no function type (`lang.mem.function_at`).
    NotAFunction(Ty),
}

/// What lowering does not handle yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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
    /// A binding of an assembly block to a place without storage, or to a symbol naming no
    /// function, link symbol or static.
    Asm,
}

impl fmt::Display for Unlowered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.why {
            Why::Unready(unready) => unready.fmt(f),
            Why::NoLayout(ty) => write!(f, "the type `{ty}` has no layout at run time"),
            Why::Unsupported(form) => write!(f, "lowering to LIR does not handle {} yet", form.what()),
            Why::Primitive(p) => write!(f, "lowering to LIR does not handle `{}` at run time yet", p.item()),
            Why::Untargeted => f.write_str("no target is chosen"),
            Why::Address(bits) => write!(f, "addresses of {bits} bits are as wide as no integer of LIR"),
            Why::Recursive => f.write_str("the coroutine's frame would hold itself"),
            Why::Unavailable(Needed::Feature(feature)) => {
                write!(f, "the instruction needs `{feature}`, which the target's processor lacks")
            }
            Why::Reported(_) => f.write_str("library code reached at run time reports an error"),
            Why::Convention(c) => {
                write!(f, "the calling convention `{c}` is not one of the target's architecture")
            }
            Why::NotAFunction(ty) => write!(f, "`{ty}` is no function type"),
            Why::Unavailable(Needed::Float(bits)) => write!(
                f,
                "the instruction needs a floating-point unit computing {bits}-bit floats, which the \
                 target's processor lacks"
            ),
        }
    }
}

impl std::error::Error for Unlowered {}

impl Why {
    /// A short name of the kind of failure, which summaries count by.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Unready(_) => "unready",
            Self::NoLayout(_) => "no layout",
            Self::Unsupported(form) => form.name(),
            Self::Primitive(_) => "primitive",
            Self::Untargeted => "untargeted",
            Self::Address(_) => "address",
            Self::Recursive => "recursive frame",
            Self::Unavailable(_) => "unavailable instruction",
            Self::Reported(_) => "reported",
            Self::Convention(_) => "convention",
            Self::NotAFunction(_) => "not a function",
        }
    }
}

impl Why {
    /// What a report says of it, in the source's terms.
    fn msg(&self) -> Msg {
        let unsupported = |what: Msg| what.text(" is not supported in compiled code yet");
        match self {
            Self::Unready(unready) => Msg::new().text(unready.to_string()),
            Self::NoLayout(ty) => Msg::new().ty(ty.to_string()).text(" has no size when the program runs"),
            Self::Unsupported(form) => unsupported(Msg::new().text(form.what())),
            Self::Primitive(p) => unsupported(Msg::new().code(p.item().to_string())),
            Self::Untargeted => Msg::new().text("no target is chosen"),
            Self::Address(bits) => Msg::new()
                .text("the target's addresses are ")
                .num(i128::from(*bits))
                .text(" bits wide, which no integer type holds"),
            Self::Recursive => Msg::new().text("this coroutine's frame would hold itself"),
            Self::Unavailable(needed) => Msg::new()
                .text("the instruction needs ")
                .append(needed.msg())
                .text(", which the target's processor lacks"),
            Self::Reported(_) => Msg::new().text("library code reached here reports an error"),
            Self::NotAFunction(ty) => {
                Msg::new().ty(ty.to_string()).text(" is no function type, which an address is made into")
            }
            Self::Convention(c) => Msg::new()
                .text("the calling convention ")
                .code(c.to_string())
                .text(" is not one of the target's architecture"),
        }
    }

    /// What to do about it, when there is a way.
    fn help(&self) -> Option<Msg> {
        match self {
            Self::NoLayout(_) | Self::Primitive(_) => {
                Some(Msg::new().text("use it only at compile time, under ").code("$"))
            }
            Self::Untargeted => Some(Msg::new().text("choose a target with ").code("--target")),
            Self::Convention(_) => Some(Msg::new().text("use ").code("#[abi.c]").text(", the target's own")),
            Self::NotAFunction(_) => Some(
                Msg::new()
                    .text("give the signature and convention of the function, as ")
                    .code("#[abi.c] (i32) -> i32"),
            ),
            Self::Recursive => Some(Msg::new().text("await the call through a pointer to its frame")),
            Self::Unready(Unready::Untyped { .. }) => Some(
                Msg::new()
                    .code("vun debug dump mir")
                    .text(" shows each local typing left without a type as ")
                    .code("?")
                    .text(", at its place"),
            ),
            _ => None,
        }
    }
}

impl Form {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Primitive => "primitive",
            Self::Conversion => "conversion",
            Self::Constant => "constant",
            Self::Callee => "callee",
            Self::Defer => "defer",
            Self::Projection => "projection",
            Self::Frame => "frame",
            Self::Quote => "quote",
            Self::Asm => "asm",
        }
    }

    #[must_use]
    pub const fn what(self) -> &'static str {
        match self {
            Self::Primitive => "this primitive at run time",
            Self::Conversion => "this conversion between types",
            Self::Constant => "this compile-time value at run time",
            Self::Callee => "a call of this kind of value",
            Self::Defer => "a deferred statement consuming linear values",
            Self::Projection => "this part of this type",
            Self::Frame => "a coroutine frame",
            Self::Quote => "a code value at run time",
            Self::Asm => "this binding of an assembly block",
        }
    }
}

impl Unlowered {
    #[must_use]
    pub fn new(at: OriginId, why: Why) -> Self {
        Self { at, why, calls: Vec::new() }
    }

    #[must_use]
    pub fn unsupported(at: OriginId, form: Form) -> Self {
        Self::new(at, Why::Unsupported(form))
    }

    /// This failure, reached through `calls`, innermost first.
    #[must_use]
    pub fn through(self, calls: Vec<OriginId>) -> Self {
        Self { calls, ..self }
    }

    /// Its report where a program is built. A library's report is shown at the outermost call
    /// that led to it, as when it is reported at compile time (spec 10, section 3); any other
    /// failure where it arises. Either lists the calls leading to it.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        let mut d = match &self.why {
            Why::Reported(reason) => {
                let trap = Trap { reason: reason.clone(), at: Some(self.at), calls: Vec::new() };
                trap.report(self.calls.last().copied().unwrap_or(self.at))
            }
            // A trap a `$` form fired in library code the program reaches, such as an error the
            // library reports for what the target lacks, is shown as it is reported at compile time.
            Why::Unready(Unready::Unevaluated { why: Unevaluated::Trapped(trap), .. }) => {
                trap.report(self.calls.last().copied().unwrap_or(self.at))
            }
            // What typing says, where it applies, rather than what it left unknown.
            Why::Unready(Unready::Untyped { said: Some(said), .. }) => (**said).clone(),
            Why::Unavailable(needed) => {
                let msg = Msg::new().text("the instruction needs ").append(needed.msg());
                Diagnostic::error(errors::ASM_UNAVAILABLE, msg.text(", which the target's processor lacks"))
                    .at(self.at, None)
                    .help(Msg::new().text("choose a target whose processor has it, with ").code("--target"))
            }
            why => {
                let d = Diagnostic::error(errors::BACKEND_UNLOWERED, why.msg()).at(self.at, None);
                match why.help() {
                    Some(help) => d.help(help),
                    None => d,
                }
            }
        };
        let shown = d.primary.as_ref().map(|p| p.origin);
        let frames = self.calls.iter().filter(|&&origin| Some(origin) != shown);
        d.trace.extend(frames.map(|&origin| TraceFrame::instantiated(origin)));
        d
    }

    /// This failure, where a primitive is what is not handled, naming it.
    #[must_use]
    pub fn naming(self, primitive: Primitive) -> Self {
        match self.why {
            Why::Unsupported(Form::Primitive) => Self { why: Why::Primitive(primitive), ..self },
            _ => self,
        }
    }
}
