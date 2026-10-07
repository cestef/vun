//! What a backend cannot express, found while emitting (spec 13, section 8); each is refused,
//! never approximated.

pub use vun_asm::AsmOperand;
use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Msg, errors};
use vun_inline::Needed;
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_target::Name;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Missing {
    Int128,
    /// A vector of 128 bits as a value of the program: only a block of assembly holds one.
    Vector,
    /// A scalar of this many bytes in one register, wider than the register.
    Wider(u64),
    Half,
    /// Floating-point values.
    Float,
    Atomic,
    /// A C record passed or returned by value.
    Record,
    /// A signature the backend would place differently from the target's convention.
    Placement,
    Naked,
    /// A call of a C function's variadic arguments.
    Variadic,
    Section,
    /// Storage per thread.
    PerThread,
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
    /// A function or call under a calling convention a function names, other than the target's
    /// own (`#[abi.stdcall]`).
    NamedConvention,
    /// A C record's eightbyte of floats holding this many bytes, neither a `f32` nor two nor a
    /// `f64`: a layout only an explicit size makes.
    FloatPart(u64),
    /// A check of each function's frame against the stack (`stack` checks).
    StackCheck,
    /// A call that replaces its caller's frame, which the backend cannot guarantee.
    TailCall,
    /// A block of inline assembly.
    Asm,
    /// An operand of inline assembly the backend cannot place.
    AsmOperand(AsmOperand),
    /// An instruction needing what the target's processor lacks, which is all the backend
    /// compiles the operation to.
    Unavailable(Needed),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
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
            vun_abi::Unpassable::Integer(bytes) => {
                Self::Width(u32::try_from(bytes * 8).expect("integers of at most 128 bits"))
            }
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
        let backend = Msg::new().text("backend ").code(backend.0);
        let what = match self {
            Missing::Unavailable(needed) => {
                let msg = backend.text(" compiles this to an instruction needing ").append(needed.msg());
                return Diagnostic::error(
                    errors::BACKEND_LACKS,
                    msg.text(", which the target's processor lacks"),
                );
            }
            Missing::Int128 => Msg::new().text("128-bit integers"),
            Missing::Vector => Msg::new().text("128-bit vectors as values"),
            Missing::Wider(n) => Msg::new().text("a value of ").num(n).text(" bytes in one register"),
            Missing::Half => Msg::new().code("f16"),
            Missing::Float => Msg::new().text("floating-point values"),
            Missing::Atomic => Msg::new().text("atomic memory access"),
            Missing::Record => Msg::new().text("C records passed by value"),
            Missing::Placement => {
                Msg::new().text("this signature placed as the target's convention places it")
            }
            Missing::Naked => Msg::new().text("naked functions"),
            Missing::Variadic => Msg::new().text("variadic arguments of C functions"),
            Missing::Section => Msg::new().text("link sections"),
            Missing::PerThread => Msg::new().text("storage per thread"),
            Missing::Align(n) => Msg::new().text("an alignment of ").num(n).text(" bytes"),
            Missing::Width(n) => Msg::new().text("an integer of ").num(n).text(" bits on this target"),
            Missing::Unaligned(n) => Msg::new().text("an address at offset ").num(n).text(" of data"),
            Missing::Symbol(s) => Msg::new().text("the link symbol ").code(s.as_str()),
            Missing::FunctionOffset => Msg::new().text("an address inside a function"),
            Missing::Format(f) => Msg::new().text("objects of format ").code(f.as_str()),
            Missing::Convention => Msg::new().text("the calling convention of this target"),
            Missing::NamedConvention => Msg::new().text("calling conventions other than the target's own"),
            Missing::StackCheck => Msg::new().text("checks of the stack's frames"),
            Missing::TailCall => Msg::new().text("guaranteed tail calls here"),
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
        let msg = backend.text(" cannot express ").append(what);
        let diagnostic = Diagnostic::error(errors::BACKEND_LACKS, msg);
        match self {
            Missing::NamedConvention => {
                diagnostic.help(Msg::new().text("build with ").code("--backend c").text(", which has them"))
            }
            _ => diagnostic,
        }
    }
}
