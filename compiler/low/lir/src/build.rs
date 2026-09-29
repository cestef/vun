//! Building a function: blocks, then instructions appended to the current one, each value typed
//! as it is made.

use vun_index::{Idx, IndexVec};
use vun_origin::OriginId;

use crate::entity::{Block, DataId, Slot, Value};
use crate::func::{BlockData, Def, Function, InstData, Linkage, Signature, SlotData, ValueData};
use crate::inst::{Access, Address, BinOp, CastOp, Cond, InstKind, Right, Terminator};
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
            stack_max: None,
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
        vun_trace::tracing::trace!(block = ?self.current, ?kind, ?ty, "instruction");
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

    /// `a op b`: a constant where both are and the operation folds, `a` itself where `b` is
    /// the constant leaving it unchanged, as `x * 1` is, a constant where `b` decides it, as `x * 0`.
    pub fn binary(&mut self, op: BinOp, a: Value, b: Value, origin: OriginId) -> Value {
        let ty = self.ty(a);
        let (ka, kb) = (self.constant_bits(a), self.constant_bits(b));
        if let Some(bits) = ka.zip(kb).and_then(|(ka, kb)| op.fold(ty, ka, kb)) {
            return self.constant(ty, bits, origin);
        }
        match kb.and_then(|kb| op.with_right(ty, kb)) {
            Some(Right::Left) => return a,
            Some(Right::Constant(bits)) => return self.constant(ty, bits, origin),
            None => {}
        }
        self.value(InstKind::Binary { op, a, b }, ty, origin)
    }

    /// The bits of `value` when a constant defines it.
    fn constant_bits(&self, value: Value) -> Option<u128> {
        self.func.constant(value).map(|(_, bits)| bits)
    }

    pub fn compare(&mut self, cond: Cond, a: Value, b: Value, origin: OriginId) -> Value {
        let ty = self.ty(a);
        let known = self.constant_bits(a).zip(self.constant_bits(b));
        if let Some(holds) = known.and_then(|(ka, kb)| cond.fold(ty, ka, kb)) {
            return self.constant(Scalar::I8, u128::from(holds), origin);
        }
        self.value(InstKind::Compare { cond, a, b }, Scalar::I8, origin)
    }

    /// `then` where `cond` is not 0, `otherwise` where it is; both of one type.
    pub fn select(&mut self, cond: Value, then: Value, otherwise: Value, origin: OriginId) -> Value {
        match self.constant_bits(cond) {
            Some(0) => return otherwise,
            Some(_) => return then,
            None => {}
        }
        let ty = self.ty(then);
        self.value(InstKind::Select { cond, then, otherwise }, ty, origin)
    }

    pub fn cast(&mut self, op: CastOp, value: Value, to: Scalar, origin: OriginId) -> Value {
        let from = self.ty(value);
        if let Some(bits) = self.constant_bits(value).and_then(|bits| op.fold(from, to, bits)) {
            return self.constant(to, bits, origin);
        }
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

    pub fn thread_addr(&mut self, data: DataId, origin: OriginId) -> Value {
        self.value(InstKind::ThreadAddr(data), Scalar::Ptr, origin)
    }

    pub fn copy(&mut self, to: Value, from: Value, bytes: u64, origin: OriginId) {
        self.push(InstKind::Copy { to, from, bytes }, None, origin);
    }

    /// Ends the current block.
    pub fn terminate(&mut self, term: Terminator) {
        vun_trace::tracing::trace!(block = ?self.current, ?term, "terminator");
        self.func.blocks[self.current].term = term;
    }

    #[must_use]
    pub fn finish(self) -> Function {
        self.func
    }
}
