//! What a backend cannot express, found while emitting (spec 13, section 8); each is refused,
//! never approximated.

use vun_diag::{Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_origin::OriginId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Int128,
    Half,
    /// Floating-point values.
    Float,
    Atomic,
    Naked,
    Section,
    Align(u64),
    /// An integer width no C integer type of the target has.
    Width(u32),
    /// An address in data at an offset the target does not align addresses to.
    Unaligned(u64),
    /// A link symbol that is not a C identifier, or is one C or the headers reserve.
    Symbol(Symbol),
    /// An address past the start of a function.
    FunctionOffset,
    /// An object format the backend does not write.
    Format(Symbol),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lack {
    pub missing: Missing,
    pub at: OriginId,
}

impl Missing {
    #[must_use]
    pub fn at(self, at: OriginId) -> Lack {
        Lack { missing: self, at }
    }
}

impl Lack {
    #[must_use]
    pub fn diagnostic(self, backend: &str) -> Diagnostic {
        self.missing.diagnostic(backend).at(self.at, None)
    }
}

impl Missing {
    /// What the backend cannot express, where no one place asks for it.
    #[must_use]
    pub fn diagnostic(self, backend: &str) -> Diagnostic {
        let what = match self {
            Missing::Int128 => Msg::new().text("128-bit integers"),
            Missing::Half => Msg::new().code("f16"),
            Missing::Float => Msg::new().text("floating-point values"),
            Missing::Atomic => Msg::new().text("atomic memory access"),
            Missing::Naked => Msg::new().text("naked functions"),
            Missing::Section => Msg::new().text("link sections"),
            Missing::Align(n) => Msg::new().text("an alignment of ").num(n).text(" bytes"),
            Missing::Width(n) => Msg::new().text("an integer of ").num(n).text(" bits on this target"),
            Missing::Unaligned(n) => Msg::new().text("an address at offset ").num(n).text(" of data"),
            Missing::Symbol(s) => Msg::new().text("the link symbol ").code(s.as_str()),
            Missing::FunctionOffset => Msg::new().text("an address inside a function"),
            Missing::Format(f) => Msg::new().text("objects of format ").code(f.as_str()),
        };
        let msg = Msg::new().text("backend ").code(backend).text(" cannot express ").append(what);
        Diagnostic::error(errors::BACKEND_LACKS, msg)
    }
}
