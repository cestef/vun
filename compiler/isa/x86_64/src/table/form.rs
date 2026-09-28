//! A form of an instruction: the kinds of operands it takes, the widths it is defined at, and its
//! encoding.

use vun_isa::imm::{fits_signed, fits_unsigned};

use crate::operand::Operand;
use crate::reg::{Reg, Size};

/// What an operand position takes. A register or memory operand without a width of its own has
/// the instruction's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    R,
    /// A general register at this width, whatever the instruction's.
    Rn(Size),
    /// Memory of the instruction's width.
    M,
    Rm,
    Rmn(Size),
    X,
    /// An SSE register, or memory of this width.
    Xmn(Size),
    /// `cl`, the count of a shift.
    Cl,
    /// 8 bits, sign-extended to the instruction's width.
    I8,
    /// The instruction's width up to 32 bits, sign-extended to 64.
    Iz,
    /// 32 bits, zero-extended to 64.
    U32,
    I64,
    /// A label or a symbol, 32 bits from the end of the instruction.
    Rel,
    Cond,
}

impl Kind {
    /// The width the operand at this position has in an instruction of `size`.
    #[must_use]
    pub const fn width(self, size: Size) -> Option<Size> {
        match self {
            Self::R | Self::Rm | Self::M => Some(size),
            Self::Rn(n) | Self::Rmn(n) | Self::Xmn(n) => Some(n),
            Self::Cl => Some(Size::B8),
            Self::X | Self::I8 | Self::Iz | Self::U32 | Self::I64 | Self::Rel | Self::Cond => None,
        }
    }

    /// The bytes of an immediate at this position, in an instruction of `size`.
    #[must_use]
    pub const fn imm_bytes(self, size: Size) -> Option<u32> {
        match self {
            Self::I8 => Some(1),
            Self::Iz => Some(if size.bytes() < 4 { size.bytes() } else { 4 }),
            Self::U32 => Some(4),
            Self::I64 => Some(8),
            _ => None,
        }
    }

    /// Whether an operand at this position is an immediate.
    #[must_use]
    pub const fn is_imm(self) -> bool {
        matches!(self, Self::I8 | Self::Iz | Self::U32 | Self::I64)
    }

    /// Whether `operand` fits this position in an instruction of `size`.
    #[must_use]
    pub fn takes<S>(self, size: Size, operand: &Operand<S>) -> bool {
        let op_bits = size.bits();
        match (self, operand) {
            (Self::R | Self::Rn(_) | Self::Rm | Self::Rmn(_), Operand::Reg(_))
            | (Self::M | Self::Rm | Self::Rmn(_) | Self::Xmn(_), Operand::Mem(_))
            | (Self::X | Self::Xmn(_), Operand::Xmm(_))
            | (Self::Rel, Operand::Label(_) | Operand::Sym(_))
            | (Self::Cond, Operand::Cond(_))
            | (Self::I64, Operand::Imm(_)) => true,
            (Self::Cl, Operand::Reg(r)) => *r == Reg::Rcx,
            (Self::I8, Operand::Imm(v)) => fits_signed(*v, 8, op_bits),
            (Self::Iz, Operand::Imm(v)) => fits_signed(*v, op_bits.min(32), op_bits),
            (Self::U32, Operand::Imm(v)) => fits_unsigned(*v, 32, op_bits),
            _ => false,
        }
    }
}

/// The widths a form is defined at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizes(u8);

impl Sizes {
    pub const ALL: Self = Self::of(&Size::ALL);
    /// Every width but 8 bits.
    pub const WORDS: Self = Self::of(&[Size::B16, Size::B32, Size::B64]);
    pub const WIDE: Self = Self::of(&[Size::B32, Size::B64]);

    #[must_use]
    pub const fn of(sizes: &[Size]) -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < sizes.len() {
            bits |= 1 << sizes[i] as u8;
            i += 1;
        }
        Self(bits)
    }

    #[must_use]
    pub const fn contains(self, size: Size) -> bool {
        self.0 & (1 << size as u8) != 0
    }
}

/// A prefix before REX: `lock`, `rep`, or the mandatory prefix of an SSE instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefix {
    None,
    Lock,
    Rep,
    P66,
    F2,
    F3,
}

impl Prefix {
    #[must_use]
    pub const fn byte(self) -> Option<u8> {
        match self {
            Self::None => None,
            Self::Lock => Some(0xf0),
            Self::P66 => Some(0x66),
            Self::F2 => Some(0xf2),
            Self::Rep | Self::F3 => Some(0xf3),
        }
    }

    /// The word written before the mnemonic, for a prefix that is an instruction's own rather
    /// than part of its opcode.
    #[must_use]
    pub const fn keyword(self) -> Option<&'static str> {
        match self {
            Self::Lock => Some("lock"),
            Self::Rep => Some("rep"),
            Self::None | Self::P66 | Self::F2 | Self::F3 => None,
        }
    }
}

/// What a form does with an operand, or with the flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
    /// Nothing but compute its address (`lea`).
    Address,
}

/// An operand position: what it takes, and what the form does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub kind: Kind,
    pub access: Access,
}

/// How the instruction's width is encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum W {
    /// `66` at 16 bits, REX.W at 64: integer operations.
    Size,
    /// REX.W at 64 only: SSE instructions over a general register of that width.
    Rex,
    Always,
    Never,
}

/// Where the operands go in the encoding, by position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// `ModRM` with a register in `reg` and a register or memory in `rm`.
    RegRm { reg: u8, rm: u8 },
    /// `ModRM` with an opcode extension in `reg`.
    Digit { digit: u8, rm: u8 },
    /// The register added to the opcode's last byte.
    PlusReg(u8),
    /// No `ModRM`.
    Bare,
}

/// Up to three opcode bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opcode {
    bytes: [u8; 3],
    len: u8,
}

impl Opcode {
    #[must_use]
    pub const fn one(a: u8) -> Self {
        Self { bytes: [a, 0, 0], len: 1 }
    }

    /// `0F` and a byte.
    #[must_use]
    pub const fn two(b: u8) -> Self {
        Self { bytes: [0x0f, b, 0], len: 2 }
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

/// One form of an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form {
    pub slots: &'static [Slot],
    pub sizes: Sizes,
    pub prefix: Prefix,
    pub opcode: Opcode,
    /// The opcode's last byte at 8 bits, where it differs.
    pub byte: Option<u8>,
    pub w: W,
    pub layout: Layout,
    /// The mnemonic, where it is not the instruction's (`movsxd`).
    pub name: Option<&'static str>,
    /// The registers it reads without naming them (`rax` and `rdx` of `div`).
    pub reads: &'static [Reg],
    pub writes: &'static [Reg],
    /// The registers it reads or writes without naming them, each once.
    pub implicit: &'static [Reg],
    /// What it does with the flags, if anything.
    pub flags: Option<Access>,
}

impl Form {
    /// The kinds of its operands, in order.
    pub fn kinds(&self) -> impl Iterator<Item = Kind> {
        self.slots.iter().map(|s| s.kind)
    }

    /// Whether this form takes `operands` at `size`.
    #[must_use]
    pub fn takes<S>(&self, size: Size, operands: &[Operand<S>]) -> bool {
        self.sizes.contains(size)
            && self.slots.len() == operands.len()
            && self.kinds().zip(operands).all(|(k, o)| k.takes(size, o))
    }
}
