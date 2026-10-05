//! The instructions, each with its forms: the opcode, the operands, and the values a form pops and
//! pushes. The table is generated from `lang.arch.wasm32`; sema's checks and the backend's encoder
//! read it, so a block that checks is one the backend encodes.

pub use crate::generated::{Feature, Op};
pub(crate) use crate::reg::Size;

/// What an operand position takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A bound local: a value read (`in`), written (`out`) or both (`inout`).
    Local,
    /// An integer immediate, LEB128 encoded.
    Imm(Imm),
}

/// An integer immediate: unsigned or signed, of this many bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Imm {
    Unsigned(u8),
    Signed(u8),
}

impl Imm {
    /// Whether the immediate holds `value`.
    #[must_use]
    pub const fn holds(self, value: i64) -> bool {
        match self {
            Self::Unsigned(bits) => value >= 0 && (bits >= 63 || value < 1 << bits),
            Self::Signed(bits) => bits >= 64 || (value >= -(1 << (bits - 1)) && value < 1 << (bits - 1)),
        }
    }
}

/// What a form does with an operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
}

/// What a stack position holds: a value type, the type of the form's local, or any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    I32,
    I64,
    F32,
    F64,
    Local,
    Any,
}

/// Where control goes after a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Next,
    Ends,
}

/// An operand position: what it takes and what the form does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arg {
    pub kind: Kind,
    pub access: Access,
}

impl Arg {
    #[must_use]
    pub const fn new(kind: Kind, access: Access) -> Self {
        Self { kind, access }
    }
}

/// What a form needs of a processor beyond WebAssembly's first version.
pub type Need = vun_isa::Need<Feature, Size>;
/// What a processor has of WebAssembly beyond its first version.
pub type Processor = vun_isa::Processor<Feature>;

/// One form of an instruction: its opcode (the prefix byte in bits 16 to 23, the code below), its
/// operands, the values it pops (the last on top) and pushes, and the bytes after its operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form {
    pub base: u32,
    pub args: &'static [Arg],
    pub pops: &'static [Ty],
    pub pushes: &'static [Ty],
    pub tail: &'static [u8],
    pub flow: Flow,
    pub requires: Option<Need>,
}

impl Form {
    #[must_use]
    pub const fn new(base: u32, args: &'static [Arg], pops: &'static [Ty], pushes: &'static [Ty]) -> Self {
        Self { base, args, pops, pushes, tail: &[], flow: Flow::Next, requires: None }
    }

    #[must_use]
    pub const fn tail(self, tail: &'static [u8]) -> Self {
        Self { tail, ..self }
    }

    #[must_use]
    pub const fn flow(self, flow: Flow) -> Self {
        Self { flow, ..self }
    }

    #[must_use]
    pub const fn requires(self, requires: Option<Need>) -> Self {
        Self { requires, ..self }
    }

    /// The prefix byte of the opcode, if it has one.
    #[must_use]
    pub fn prefix(&self) -> Option<u8> {
        match u8::try_from(self.base >> 16) {
            Ok(0) | Err(_) => None,
            Ok(prefix) => Some(prefix),
        }
    }

    /// The code of the opcode, after its prefix.
    #[must_use]
    pub const fn code(&self) -> u32 {
        self.base & 0xffff
    }
}

impl Op {
    /// The instruction a block names, and the form it was checked against.
    #[must_use]
    pub fn entry(op: vun_inline::OpId, form: vun_inline::FormId) -> (Self, &'static Form) {
        let op = Self::ALL[usize::from(op.0)];
        (op, &op.forms()[usize::from(form.0)])
    }
}
