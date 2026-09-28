//! How an operand is written, as assemblers group the operands a mnemonic's encodings take: a
//! 32-bit form is written with `.w` where a 16-bit form of its mnemonic takes operands written
//! alike, as assemblers otherwise choose the shorter.

use super::kind::Kind;
use super::{Form, Halves, Op};
use crate::generated::Mode;

/// A written shape of an operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Syntax {
    Reg,
    Imm,
    /// `[base, #offset]` with a positive offset.
    Offset,
    /// `[base, index]`.
    Index,
    /// Any other memory: a negative offset, or written back.
    Memory,
    List,
    Target,
    /// A register of another kind, a condition or a choice, which no two forms write apart.
    Other,
}

impl Kind {
    const fn syntax(self) -> Syntax {
        match self {
            Self::Gpr(_) | Self::Split | Self::Shifted => Syntax::Reg,
            Self::Imm(_) => Syntax::Imm,
            Self::Mem(Mode::Narrow | Mode::NarrowSp | Mode::Offset, _) => Syntax::Offset,
            Self::Mem(Mode::NarrowIndex | Mode::Index, _) => Syntax::Index,
            Self::Mem(..) => Syntax::Memory,
            Self::List(_) => Syntax::List,
            Self::Target(_) => Syntax::Target,
            _ => Syntax::Other,
        }
    }
}

/// How a form's operands are written.
trait Written {
    fn syntax(&self) -> impl Iterator<Item = Syntax> + '_;

    /// Whether this 16-bit form is written as `wide` is: its operands alike, or all but the first,
    /// which a 16-bit form of two operands reads and writes.
    fn written_as(&self, wide: &Self) -> bool;
}

impl Written for Form {
    fn syntax(&self) -> impl Iterator<Item = Syntax> + '_ {
        self.kinds().map(Kind::syntax)
    }

    fn written_as(&self, wide: &Self) -> bool {
        self.syntax().eq(wide.syntax()) || self.syntax().eq(wide.syntax().skip(1))
    }
}

impl Op {
    /// Whether `form`, one of this instruction's, is written with `.w`: a 32-bit form one of its
    /// 16-bit forms is written as.
    #[must_use]
    pub fn qualified(self, form: &Form) -> bool {
        form.wide() && self.forms().iter().any(|narrow| !narrow.wide() && narrow.written_as(form))
    }
}
