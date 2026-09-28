//! Operands as a line gives them, before a form takes them: words read as the architecture's
//! registers, conditions and choices, integers as immediates, labels as the lines they mark, and
//! bindings with the class and width of register their value needs.

use vun_codec::{Decode, Encode};
use vun_hir::tree::ExprId;
use vun_inline::{Base, Bound, CondId, RegRef, SizeId};
use vun_isa::ChoiceId;

/// An operand as written, as the architectures' assemblers fit it.
pub(crate) type Given = vun_isa::Given<ExprId, ExprId, ExprId>;

/// A word of an architecture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Word {
    /// A register, with the width its name gives, if it gives one.
    Reg { reg: RegRef, size: Option<SizeId> },
    /// A condition written as an operand (`csel x0, x1, x2, eq`).
    Cond(CondId),
    /// One of the choices of a position: its kind, and its index among that kind's (`ish` of a
    /// barrier).
    Choice { of: ChoiceId, index: u8 },
}

impl Word {
    /// The operand it is.
    pub(crate) const fn given(self) -> Given {
        match self {
            Self::Reg { reg, size } => Given::Reg { reg, size },
            Self::Cond(cond) => Given::Cond(cond),
            Self::Choice { of, index } => Given::Choice { of, index },
        }
    }
}

/// What an operand is as the register of an address.
pub(crate) trait Addressing {
    /// A register or a value read in one, as the base or index of an address.
    fn base(self) -> Option<Base<ExprId, ExprId>>;
}

impl Addressing for Given {
    fn base(self) -> Option<Base<ExprId, ExprId>> {
        match self {
            Self::Reg { reg, .. } => Some(Base::Reg(reg)),
            Self::Bound { bound: bound @ Bound::In { .. }, .. } => Some(Base::Bound(bound)),
            _ => None,
        }
    }
}
