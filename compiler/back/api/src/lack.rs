//! What a backend cannot express, found while emitting (spec 13, section 8); each is refused,
//! never approximated.

pub use vun_asm::AsmOperand;
use vun_diag::{Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_target::Name;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Int128,
    /// A scalar of this many bytes in one register, wider than the register.
    Wider(u64),
    Half,
    /// Floating-point values.
    Float,
    Atomic,
    Naked,
    /// A call of a C function's variadic arguments.
    Variadic,
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
    /// The target's calling convention.
    Convention,
    /// A C record's eightbyte of floats holding this many bytes, neither a `f32` nor two nor a
    /// `f64`: a layout only an explicit size makes.
    FloatPart(u64),
    /// A block of inline assembly.
    Asm,
    /// An operand of inline assembly the backend cannot place.
    AsmOperand(AsmOperand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lack {
    pub missing: Missing,
    pub at: OriginId,
}

impl From<vun_abi::Unpassable> for Missing {
    fn from(unpassable: vun_abi::Unpassable) -> Self {
        match unpassable {
            vun_abi::Unpassable::Half => Self::Half,
            vun_abi::Unpassable::Wider(bytes) => Self::Wider(bytes),
            vun_abi::Unpassable::FloatPart(bytes) => Self::FloatPart(bytes),
            vun_abi::Unpassable::Naked => Self::Naked,
        }
    }
}

impl Missing {
    #[must_use]
    pub fn at(self, at: OriginId) -> Lack {
        Lack { missing: self, at }
    }
}

impl Lack {
    #[must_use]
    pub fn diagnostic(self, backend: Name) -> Diagnostic {
        self.missing.diagnostic(backend).at(self.at, None)
    }
}

impl Missing {
    /// What the backend cannot express, where no one place asks for it.
    #[must_use]
    pub fn diagnostic(self, backend: Name) -> Diagnostic {
        let what = match self {
            Missing::Int128 => Msg::new().text("128-bit integers"),
            Missing::Wider(n) => Msg::new().text("a value of ").num(n).text(" bytes in one register"),
            Missing::Half => Msg::new().code("f16"),
            Missing::Float => Msg::new().text("floating-point values"),
            Missing::Atomic => Msg::new().text("atomic memory access"),
            Missing::Naked => Msg::new().text("naked functions"),
            Missing::Variadic => Msg::new().text("variadic arguments of C functions"),
            Missing::Section => Msg::new().text("link sections"),
            Missing::Align(n) => Msg::new().text("an alignment of ").num(n).text(" bytes"),
            Missing::Width(n) => Msg::new().text("an integer of ").num(n).text(" bits on this target"),
            Missing::Unaligned(n) => Msg::new().text("an address at offset ").num(n).text(" of data"),
            Missing::Symbol(s) => Msg::new().text("the link symbol ").code(s.as_str()),
            Missing::FunctionOffset => Msg::new().text("an address inside a function"),
            Missing::Format(f) => Msg::new().text("objects of format ").code(f.as_str()),
            Missing::Convention => Msg::new().text("the calling convention of this target"),
            Missing::Asm => Msg::new().text("inline assembly"),
            Missing::AsmOperand(operand) => {
                let what = match operand {
                    AsmOperand::Indexed => "memory with an index register",
                    AsmOperand::SymbolOffRegister => "memory at a symbol's address past a register",
                    AsmOperand::Absolute => "memory at a fixed address",
                    AsmOperand::Imported => "memory at a symbol another module defines",
                    AsmOperand::Unallocated => "a value bound to a register of this class",
                    AsmOperand::Unencodable => "operands the instruction's form does not encode",
                    AsmOperand::Symbol => "a symbol",
                    AsmOperand::Crowded => "more bindings than free registers",
                };
                Msg::new().text(what).text(" in inline assembly")
            }
            Missing::FloatPart(n) => {
                Msg::new().text("a C record passing ").num(n).text(" bytes of floats in a register")
            }
        };
        let msg = Msg::new().text("backend ").code(backend.0).text(" cannot express ").append(what);
        Diagnostic::error(errors::BACKEND_LACKS, msg)
    }
}
