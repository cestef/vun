//! Functions: signatures, stack slots, blocks with parameters, instructions and the values they
//! define. Every block, instruction, slot and the function itself carry an origin.

use vun_codec::{Decode, Encode};
use vun_index::IndexVec;
use vun_intern::Symbol;
use vun_origin::OriginId;

use crate::entity::{Block, Inst, Slot, Value};
use crate::inst::{Address, InstKind, Terminator};
use crate::ty::Scalar;

pub use crate::abi::{AbiType, Alias, Conv, Ext, Held, Record, Reuse, Signature};

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
    /// Under the `stack` checks, how the frame is checked before the stack moves to it.
    pub stack_check: Option<StackCheck>,
}

/// How a frame is checked under the `stack` checks (spec 13, section 8.7): the runtime's
/// `lang.check.stack(frame, site)`, called with the lowest address of the frame before the stack
/// moves to it, and the site in the message table it fails at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct StackCheck {
    pub site: u32,
    pub call: Address,
}

impl Function {
    /// Replaces each function, datum and import its instructions name by what `f` gives: the
    /// ids of another unit it is placed in.
    pub fn map_addresses(&mut self, mut f: impl FnMut(Address) -> Address) {
        for (_, inst) in self.insts.iter_mut() {
            inst.kind.map_addresses(&mut f);
        }
        if let Some(check) = &mut self.stack_check {
            check.call = f(check.call);
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
