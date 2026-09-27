//! Operands as a line gives them, before a form takes them: words read as the architecture's
//! registers, integers as immediates, labels as the lines they mark, and bindings with the class
//! and width of register their value needs.

use vun_hir::tree::ExprId;
use vun_inline::{Base, Bound, ClassId, CondId, Line, Mem, Operand, RegRef, SizeId};

/// A word of an architecture: a register, with the width its name gives, if it gives one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Word {
    Reg { reg: RegRef, size: Option<SizeId> },
}

/// An operand as written.
#[derive(Clone, Debug)]
pub(crate) enum Given {
    Reg {
        reg: RegRef,
        size: Option<SizeId>,
    },
    /// A value bound to a register of the class and width its type needs, or to the one named.
    Bound {
        bound: Bound<ExprId, ExprId>,
        class: ClassId,
        size: Option<SizeId>,
    },
    /// An integer; `None` when no 64 bits hold it.
    Imm(Option<i64>),
    Cond(CondId),
    Mem(Mem<ExprId, ExprId, ExprId>),
    Label(Line),
    /// The address of a declaration.
    Symbol(ExprId),
}

impl Given {
    /// The register class it names or binds, if it is a register.
    pub(crate) fn class(&self) -> Option<ClassId> {
        match self {
            Self::Reg { reg, .. } => Some(reg.class),
            Self::Bound { class, .. } => Some(*class),
            _ => None,
        }
    }

    /// The width of register it names or binds, if it is a register of a known width.
    pub(crate) fn size(&self) -> Option<SizeId> {
        match self {
            Self::Reg { size, .. } | Self::Bound { size, .. } => *size,
            _ => None,
        }
    }

    /// A register or a value read in one, as the base or index of an address.
    pub(crate) fn base(self) -> Option<Base<ExprId, ExprId>> {
        match self {
            Self::Reg { reg, .. } => Some(Base::Reg(reg)),
            Self::Bound { bound: bound @ Bound::In { .. }, .. } => Some(Base::Bound(bound)),
            _ => None,
        }
    }

    /// The operand of a checked instruction it is.
    pub(crate) fn operand(self) -> Operand<ExprId, ExprId, ExprId> {
        match self {
            Self::Reg { reg, .. } => Operand::Reg(reg),
            Self::Bound { bound, .. } => Operand::Bound(bound),
            Self::Imm(imm) => Operand::Imm(imm.unwrap_or_default()),
            Self::Cond(cond) => Operand::Cond(cond),
            Self::Mem(mem) => Operand::Mem(mem),
            Self::Label(line) => Operand::Label(line),
            Self::Symbol(symbol) => Operand::Symbol(symbol),
        }
    }
}
