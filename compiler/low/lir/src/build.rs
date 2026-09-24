//! Building a function: blocks, then instructions appended to the current one, each value typed
//! as it is made.

use vun_index::{Idx, IndexVec};
use vun_origin::OriginId;

use crate::entity::{Block, Slot, Value};
use crate::func::{BlockData, Def, Function, InstData, Linkage, Signature, SlotData, ValueData};
use crate::inst::{Access, Address, BinOp, CastOp, Cond, InstKind, Terminator};
use crate::ty::Scalar;

pub struct Builder {
    func: Function,
    current: Block,
}

impl Builder {
    /// A function whose entry block receives the signature's values, building in the entry.
    #[must_use]
    pub fn new(linkage: Linkage, signature: Signature, origin: OriginId) -> Self {
        let params = signature.values();
        let func = Function {
            linkage,
            signature,
            slots: IndexVec::new(),
            blocks: IndexVec::new(),
            insts: IndexVec::new(),
            values: IndexVec::new(),
            origin,
        };
        let mut b = Self { func, current: Idx::new(0) };
        b.current = b.block(&params, origin);
        b
    }

    /// A new block with parameters of the types `params`, ending unreachably until terminated.
    pub fn block(&mut self, params: &[Scalar], origin: OriginId) -> Block {
        let block = self.func.blocks.push(BlockData {
            params: Vec::new(),
            insts: Vec::new(),
            term: Terminator::Unreachable,
            origin,
        });
        let values = params
            .iter()
            .enumerate()
            .map(|(i, &ty)| self.func.values.push(ValueData { ty, def: Def::Param(block, i) }))
            .collect();
        self.func.blocks[block].params = values;
        block
    }

    #[must_use]
    pub fn params(&self, block: Block) -> &[Value] {
        &self.func.blocks[block].params
    }

    /// The block instructions are appended to.
    #[must_use]
    pub fn current(&self) -> Block {
        self.current
    }

    pub fn switch_to(&mut self, block: Block) {
        self.current = block;
    }

    pub fn slot(&mut self, size: u64, align: u64, origin: OriginId) -> Slot {
        self.func.slots.push(SlotData { size, align, origin })
    }

    #[must_use]
    pub fn ty(&self, value: Value) -> Scalar {
        self.func.ty(value)
    }

    /// Appends `kind`, defining a value of type `ty` when it has one.
    pub fn push(&mut self, kind: InstKind, ty: Option<Scalar>, origin: OriginId) -> Option<Value> {
        let inst = self.func.insts.push(InstData { kind, result: None, origin });
        let result = ty.map(|ty| self.func.values.push(ValueData { ty, def: Def::Inst(inst) }));
        self.func.insts[inst].result = result;
        self.func.blocks[self.current].insts.push(inst);
        result
    }

    /// Appends `kind`, which defines a value of type `ty`.
    fn value(&mut self, kind: InstKind, ty: Scalar, origin: OriginId) -> Value {
        self.push(kind, Some(ty), origin).expect("a typed instruction defines its value")
    }

    pub fn constant(&mut self, ty: Scalar, bits: u128, origin: OriginId) -> Value {
        self.value(InstKind::Const { ty, bits }, ty, origin)
    }

    pub fn binary(&mut self, op: BinOp, a: Value, b: Value, origin: OriginId) -> Value {
        let ty = self.ty(a);
        self.value(InstKind::Binary { op, a, b }, ty, origin)
    }

    pub fn compare(&mut self, cond: Cond, a: Value, b: Value, origin: OriginId) -> Value {
        self.value(InstKind::Compare { cond, a, b }, Scalar::I8, origin)
    }

    pub fn cast(&mut self, op: CastOp, value: Value, to: Scalar, origin: OriginId) -> Value {
        self.value(InstKind::Cast { op, value, to }, to, origin)
    }

    pub fn load(&mut self, ty: Scalar, addr: Value, access: Access, origin: OriginId) -> Value {
        self.value(InstKind::Load { ty, addr, access }, ty, origin)
    }

    pub fn store(&mut self, value: Value, addr: Value, access: Access, origin: OriginId) {
        self.push(InstKind::Store { value, addr, access }, None, origin);
    }

    pub fn offset(&mut self, base: Value, offset: Value, origin: OriginId) -> Value {
        self.value(InstKind::Offset { base, offset }, Scalar::Ptr, origin)
    }

    pub fn slot_addr(&mut self, slot: Slot, origin: OriginId) -> Value {
        self.value(InstKind::SlotAddr(slot), Scalar::Ptr, origin)
    }

    pub fn addr(&mut self, address: Address, origin: OriginId) -> Value {
        self.value(InstKind::Addr(address), Scalar::Ptr, origin)
    }

    pub fn copy(&mut self, to: Value, from: Value, bytes: u64, origin: OriginId) {
        self.push(InstKind::Copy { to, from, bytes }, None, origin);
    }

    /// Ends the current block.
    pub fn terminate(&mut self, term: Terminator) {
        self.func.blocks[self.current].term = term;
    }

    #[must_use]
    pub fn finish(self) -> Function {
        self.func
    }
}
