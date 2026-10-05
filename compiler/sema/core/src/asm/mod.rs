//! Blocks of an architecture's instructions (spec 07, section 6), checked against the
//! architecture's table in `lang.arch`: each call of a block names instructions, the parts it
//! writes select their forms, and the first whose operands fit is the instruction's, as the native
//! assemblers choose. What the table does not say, how memory is written and which register a
//! value is held in, each architecture tells here.

mod arm64;
mod bracket;
mod check;
mod given;
mod inst;
mod need;
mod parts;
mod read;
mod stack;
mod table;
mod thumb;
mod wasm32;
mod x86_64;

pub(crate) use check::Checker;
pub use table::{ArchTable, Flow, Op, Table};
use vun_codec::{Decode, Encode};
use vun_hir::tree::ExprId;
use vun_items::{Item, lang};

/// A block as sema checked it: each operand's binding is the expression it binds.
pub type Block = vun_inline::Block<ExprId, ExprId, ExprId>;

/// The architectures whose instructions a block holds, each with its table in `lang.arch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Arch {
    X86_64,
    Arm64,
    Thumb,
    Wasm32,
}

impl Arch {
    pub const ALL: &[Self] = &[Self::X86_64, Self::Arm64, Self::Thumb, Self::Wasm32];

    /// The namespace declaring its table.
    #[must_use]
    pub const fn namespace(self) -> Item {
        match self {
            Self::X86_64 => lang::arch::x86_64,
            Self::Arm64 => lang::arch::arm64,
            Self::Thumb => lang::arch::thumb,
            Self::Wasm32 => lang::arch::wasm32,
        }
    }

    /// The declarations its table is read through.
    const fn shape(self) -> &'static table::Shape {
        match self {
            Self::X86_64 => &x86_64::SHAPE,
            Self::Arm64 => &arm64::SHAPE,
            Self::Thumb => &thumb::SHAPE,
            Self::Wasm32 => &wasm32::SHAPE,
        }
    }
}
