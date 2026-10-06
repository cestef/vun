//! The instructions of a block of `lang.arch.wasm32`, as the module holds them: the form of the
//! instruction its table declares, with its operands.

use vun_inline::{FormId, OpId};
use vun_isa_wasm32::{Class, Ty, Yields};

use super::{SymbolId, ValType};

/// An operand of an instruction of a block of `lang.arch.wasm32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operand {
    /// A local of the function, by its index.
    Local(u32),
    /// An integer immediate.
    Imm(i64),
    /// One of a kind of named choice, by its index.
    Choice(u8),
    /// A function called or a global read or written, by its symbol.
    Symbol(SymbolId),
}

impl Asm {
    /// The branch depths its operands give, in order.
    pub fn depths(&self) -> impl Iterator<Item = u32> + '_ {
        self.operands.iter().filter_map(|o| match o {
            Operand::Imm(depth) => u32::try_from(*depth).ok(),
            _ => None,
        })
    }

    /// The symbol its operands name, if one.
    #[must_use]
    pub fn symbol(&self) -> Option<SymbolId> {
        self.operands.iter().find_map(|o| match o {
            Operand::Symbol(symbol) => Some(*symbol),
            _ => None,
        })
    }

    /// The type of the value the block it opens gives, where its operands name one.
    #[must_use]
    pub fn yields(&self) -> Option<ValType> {
        self.operands.iter().find_map(|o| match o {
            Operand::Choice(at) => Yields::ALL[usize::from(*at)].class().val_type(),
            _ => None,
        })
    }
}

/// The value type a table's class of locals or stack position holds.
pub trait Typed {
    /// The type, where there is one: not where the type is the local's, any or unknown.
    fn val_type(self) -> Option<ValType>;
}

impl Typed for Class {
    fn val_type(self) -> Option<ValType> {
        match self {
            Self::I32 => Some(ValType::I32),
            Self::I64 => Some(ValType::I64),
            Self::F32 => Some(ValType::F32),
            Self::F64 => Some(ValType::F64),
            Self::V128 => Some(ValType::V128),
            Self::Unknown => None,
        }
    }
}

impl Typed for Ty {
    fn val_type(self) -> Option<ValType> {
        match self {
            Self::I32 => Some(ValType::I32),
            Self::I64 => Some(ValType::I64),
            Self::F32 => Some(ValType::F32),
            Self::F64 => Some(ValType::F64),
            Self::V128 => Some(ValType::V128),
            Self::Local | Self::Any => None,
        }
    }
}

/// An instruction of a block of `lang.arch.wasm32`: the form of the instruction its table
/// declares, which says its opcode and what it pops and pushes, with its operands.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Asm {
    pub op: OpId,
    pub form: FormId,
    pub operands: Vec<Operand>,
}
