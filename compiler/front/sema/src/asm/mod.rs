//! Blocks of an architecture's instructions (spec 07, section 6), checked against the
//! architecture's table in `lang.arch`: a line's mnemonic names candidate forms, and the first
//! whose operands fit is the instruction's, as the native assemblers choose. What the table does
//! not say, how an operand of each kind is written, each architecture tells here.

mod arm64;
mod check;
mod given;
mod read;
mod table;
mod x86_64;

use vun_codec::{Decode, Encode};
use vun_hir::tree::ExprId;
use vun_items::{Item, lang};

pub(crate) use check::Checker;
pub use given::Word;
pub use table::{ArchTable, Flow, Mnemonic, Op, Spelling, Table};

/// A block as sema checked it: each operand's binding is the expression it binds.
pub type Block = vun_inline::Block<ExprId, ExprId, ExprId>;

/// The architectures whose instructions a block holds, each with its table in `lang.arch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Arch {
    X86_64,
    Arm64,
}

impl Arch {
    pub const ALL: &[Self] = &[Self::X86_64, Self::Arm64];

    /// The namespace declaring its table.
    #[must_use]
    pub const fn namespace(self) -> Item {
        match self {
            Self::X86_64 => lang::arch::x86_64,
            Self::Arm64 => lang::arch::arm64,
        }
    }

    /// The declarations its table is read through.
    const fn shape(self) -> &'static table::Shape {
        match self {
            Self::X86_64 => &x86_64::SHAPE,
            Self::Arm64 => &arm64::SHAPE,
        }
    }
}
