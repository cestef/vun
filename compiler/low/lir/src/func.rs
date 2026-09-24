//! Functions: signatures, stack slots, blocks with parameters, instructions and the values they
//! define. Every block, instruction, slot and the function itself carry an origin.

use vun_index::IndexVec;
use vun_intern::Symbol;
use vun_origin::OriginId;

use crate::entity::{Block, Inst, Slot, Value};
use crate::inst::{InstKind, Terminator};
use crate::ty::Scalar;

/// How a function is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Conv {
    /// Vun's own: scalars as they are; aggregates by address, a result through an address
    /// passed first. Backends choose registers freely.
    Vun,
    /// The target's C calling convention (`#[abi.c]`): records are passed and returned by value,
    /// as C passes structs.
    C,
    /// No prologue or epilogue (`#[abi.naked]`): the body is its own entry.
    Naked,
}

/// A value a signature passes: a scalar; a record passed by value under the C convention; or a
/// value in memory passed by address under Vun's. The body and callers hold both in memory,
/// reaching them by address.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AbiType {
    Scalar(Scalar),
    Record(Record),
    Memory { size: u64, align: u64 },
}

/// A record as C lays it out: its size, alignment and the scalars at their offsets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Record {
    pub size: u64,
    pub align: u64,
    pub fields: Vec<(u64, Scalar)>,
}

impl AbiType {
    /// The type of the LIR value standing for it: the scalar, or the address of the record.
    #[must_use]
    pub fn value(&self) -> Scalar {
        match self {
            Self::Scalar(s) => *s,
            Self::Record(_) | Self::Memory { .. } => Scalar::Ptr,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Signature {
    pub conv: Conv,
    pub params: Vec<AbiType>,
    /// The result; one in memory is written through the address the body receives after its
    /// parameters, and callers pass.
    pub result: Option<AbiType>,
}

impl Signature {
    /// The types of the values the entry block receives and a call passes: the parameters, then
    /// the address of a result in memory.
    #[must_use]
    pub fn values(&self) -> Vec<Scalar> {
        let out = self.through().then_some(Scalar::Ptr);
        self.params.iter().map(AbiType::value).chain(out).collect()
    }

    /// Whether the result is written through an address rather than returned.
    #[must_use]
    pub fn through(&self) -> bool {
        matches!(self.result, Some(AbiType::Record(_) | AbiType::Memory { .. }))
    }

    /// The type of the value a call gives, for a scalar result.
    #[must_use]
    pub fn returns(&self) -> Option<Scalar> {
        match &self.result {
            Some(AbiType::Scalar(s)) => Some(*s),
            Some(AbiType::Record(_) | AbiType::Memory { .. }) | None => None,
        }
    }
}

/// Who sees a definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Linkage {
    /// The unit only; backends name it as they choose.
    Internal,
    /// Exported under the symbol (`#[link.export]`).
    Export(Symbol),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotData {
    pub size: u64,
    pub align: u64,
    pub origin: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlockData {
    pub params: Vec<Value>,
    pub insts: Vec<Inst>,
    pub term: Terminator,
    pub origin: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InstData {
    pub kind: InstKind,
    pub result: Option<Value>,
    pub origin: OriginId,
}

/// Where a value is defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Def {
    /// The parameter at a position of a block.
    Param(Block, usize),
    Inst(Inst),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueData {
    pub ty: Scalar,
    pub def: Def,
}

/// A function with a body. The entry is the first block, whose parameters are the signature's
/// values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Function {
    pub linkage: Linkage,
    pub signature: Signature,
    pub slots: IndexVec<Slot, SlotData>,
    pub blocks: IndexVec<Block, BlockData>,
    pub insts: IndexVec<Inst, InstData>,
    pub values: IndexVec<Value, ValueData>,
    pub origin: OriginId,
}

impl Function {
    /// The block control starts in.
    #[must_use]
    pub fn entry(&self) -> Option<Block> {
        self.blocks.iter().map(|(b, _)| b).next()
    }

    #[must_use]
    pub fn ty(&self, value: Value) -> Scalar {
        self.values[value].ty
    }
}
