//! Functions: signatures, stack slots, blocks with parameters, instructions and the values they
//! define. Every block, instruction, slot and the function itself carry an origin.

use std::iter;

use vun_codec::{Decode, Encode};
use vun_index::IndexVec;
use vun_intern::Symbol;
use vun_origin::OriginId;

use crate::entity::{Block, Inst, Slot, Value};
use crate::inst::{Address, InstKind, Terminator};
use crate::ty::Scalar;

/// How a function is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum AbiType {
    /// A float.
    Scalar(Scalar),
    /// An address, and whether anything else reaches what it does while the call runs.
    Address(Alias),
    /// An integer, and how it extends to a wider register, as its range reads it: C declares it
    /// signed or unsigned by that.
    Int(Scalar, Ext),
    /// An integer twice as wide as `half`, passed as its halves, low first, which the backend's
    /// call convention places together; `ext` is how the whole extends.
    Wide {
        half: Scalar,
        ext: Ext,
    },
    Record(Record),
    Memory {
        size: u64,
        align: u64,
    },
}

/// What else reaches what an address passed to a call reaches, while the call runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Alias {
    /// Anything may.
    Any,
    /// Nothing but this address and those made from it: an exclusive borrow (spec 05, section 8).
    Exclusive,
}

/// How an integer extends to a wider register: with zeros, or copies of its sign bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Ext {
    Zero,
    Sign,
}

impl Ext {
    /// The bits of the integer `bits` of type `from`, extended this way to 64 bits.
    #[must_use]
    pub fn widen(self, from: Scalar, bits: u128) -> u128 {
        match self {
            Self::Zero => bits,
            Self::Sign => Scalar::I64.truncate(from.signed(bits)),
        }
    }
}

/// A record as C lays it out: its size, alignment and the scalars at their offsets.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Record {
    pub size: u64,
    pub align: u64,
    pub fields: Vec<(u64, Scalar)>,
}

impl AbiType {
    /// The types of the LIR values standing for it: the scalar, the halves of a wide integer,
    /// or the address of the record.
    pub fn values(&self) -> impl Iterator<Item = Scalar> + use<> {
        let (scalar, count) = match self {
            Self::Scalar(s) | Self::Int(s, _) => (*s, 1),
            Self::Wide { half, .. } => (*half, 2),
            Self::Address(_) | Self::Record(_) | Self::Memory { .. } => (Scalar::Ptr, 1),
        };
        iter::repeat_n(scalar, count)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Signature {
    pub conv: Conv,
    pub params: Vec<AbiType>,
    /// The result; one in memory is written through the address the body receives after its
    /// parameters, and callers pass.
    pub result: Option<AbiType>,
    /// Under the C convention, the parameter the variadic arguments start at (C's `...`); the
    /// function is an import declared at the arity its callers use.
    pub variadic: Option<usize>,
}

impl Signature {
    /// The types of the values the entry block receives and a call passes: the parameters, then
    /// the address of a result in memory.
    #[must_use]
    pub fn values(&self) -> Vec<Scalar> {
        let out = self.through().then_some(Scalar::Ptr);
        self.params.iter().flat_map(AbiType::values).chain(out).collect()
    }

    /// The parameter each LIR value of [`Signature::values`] stands for, but the address of a
    /// result in memory: a wide integer's twice.
    pub fn passed(&self) -> impl Iterator<Item = &AbiType> {
        self.params.iter().flat_map(|p| iter::repeat_n(p, p.values().count()))
    }

    /// Whether every integer it passes says how it extends, and every address what aliases it.
    #[must_use]
    pub fn well_formed(&self) -> bool {
        self.params.iter().chain(&self.result).all(|p| match p {
            AbiType::Scalar(s) => s.is_float(),
            AbiType::Int(s, _) | AbiType::Wide { half: s, .. } => s.is_int(),
            AbiType::Address(_) | AbiType::Record(_) | AbiType::Memory { .. } => true,
        })
    }

    /// Whether it marks variadic arguments only as C passes them: under the C convention, from
    /// one of its parameters.
    #[must_use]
    pub fn variadic_well_formed(&self) -> bool {
        self.variadic.is_none_or(|fixed| self.conv == Conv::C && fixed <= self.params.len())
    }

    /// Whether the result is written through an address rather than returned as one value: a
    /// record, memory, or a wide integer's halves, which the convention may return in registers
    /// the caller then writes there.
    #[must_use]
    pub fn through(&self) -> bool {
        matches!(self.result, Some(AbiType::Record(_) | AbiType::Wide { .. } | AbiType::Memory { .. }))
    }

    /// The type of the value a call gives, for a scalar result.
    #[must_use]
    pub fn returns(&self) -> Option<Scalar> {
        match &self.result {
            Some(AbiType::Scalar(s) | AbiType::Int(s, _)) => Some(*s),
            Some(AbiType::Address(_)) => Some(Scalar::Ptr),
            Some(AbiType::Wide { .. } | AbiType::Record(_) | AbiType::Memory { .. }) | None => None,
        }
    }
}

/// Who sees a definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Linkage {
    /// The unit only; backends name it as they choose.
    Internal,
    /// Exported under the symbol (`#[link.export]`).
    Export(Symbol),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct SlotData {
    pub size: u64,
    pub align: u64,
    pub origin: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct BlockData {
    pub params: Vec<Value>,
    pub insts: Vec<Inst>,
    pub term: Terminator,
    pub origin: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct InstData {
    pub kind: InstKind,
    pub result: Option<Value>,
    pub origin: OriginId,
}

/// Where a value is defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Def {
    /// The parameter at a position of a block.
    Param(Block, usize),
    Inst(Inst),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct ValueData {
    pub ty: Scalar,
    pub def: Def,
}

/// A function with a body. The entry is the first block, whose parameters are the signature's
/// values.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Function {
    pub linkage: Linkage,
    pub signature: Signature,
    pub slots: IndexVec<Slot, SlotData>,
    pub blocks: IndexVec<Block, BlockData>,
    pub insts: IndexVec<Inst, InstData>,
    pub values: IndexVec<Value, ValueData>,
    pub origin: OriginId,
    /// The most bytes of stack a call of the function may take, with the calls it makes, when
    /// the function is bound (`#[stack.max(n)]`, spec 07, section 4).
    pub stack_max: Option<u64>,
}

impl Function {
    /// Replaces each function, datum and import its instructions name by what `f` gives: the
    /// ids of another unit it is placed in.
    pub fn map_addresses(&mut self, mut f: impl FnMut(Address) -> Address) {
        for (_, inst) in self.insts.iter_mut() {
            inst.kind.map_addresses(&mut f);
        }
    }

    /// The block control starts in.
    #[must_use]
    pub fn entry(&self) -> Option<Block> {
        self.blocks.iter().map(|(b, _)| b).next()
    }

    #[must_use]
    pub fn ty(&self, value: Value) -> Scalar {
        self.values[value].ty
    }

    /// A new instruction of `kind`, in no block yet, defining a value of type `ty` if it has one.
    pub fn define(&mut self, kind: InstKind, ty: Option<Scalar>, origin: OriginId) -> (Inst, Option<Value>) {
        let inst = self.insts.push(InstData { kind, result: None, origin });
        let result = ty.map(|ty| self.values.push(ValueData { ty, def: Def::Inst(inst) }));
        self.insts[inst].result = result;
        (inst, result)
    }

    /// A new instruction of `kind` appended to `insts`, a block's being built; gives its value.
    pub fn emit(
        &mut self,
        kind: InstKind,
        ty: Option<Scalar>,
        origin: OriginId,
        insts: &mut Vec<Inst>,
    ) -> Option<Value> {
        let (inst, value) = self.define(kind, ty, origin);
        insts.push(inst);
        value
    }

    /// The type and bits of `value` when a constant defines it.
    #[must_use]
    pub fn constant(&self, value: Value) -> Option<(Scalar, u128)> {
        let Def::Inst(inst) = self.values[value].def else { return None };
        match self.insts[inst].kind {
            InstKind::Const { ty, bits } => Some((ty, bits)),
            _ => None,
        }
    }
}
