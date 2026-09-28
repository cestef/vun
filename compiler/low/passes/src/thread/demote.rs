//! Demoting a block's parameters to slots, so jumps can go past the block though what follows
//! it reads them: each jump to it stores its arguments, and each read of a parameter loads it
//! where it is read. Promotion makes them values again, with parameters where definitions now
//! meet.

use vun_hash::Map;
use vun_origin::OriginId;

use vun_lir::entity::{Block, Inst, Slot, Value};
use vun_lir::func::{Function, SlotData};
use vun_lir::inst::{Access, InstKind, Terminator};
use vun_lir::ty::Scalar;

pub(crate) trait Demote {
    /// Demotes the parameters of `block`, every jump to which is a `Jump`, to slots, giving
    /// each parameter's.
    fn demote_params(&mut self, block: Block, pointer_bits: u32) -> Map<Value, Slot>;

    /// Appends to `insts` a load of each parameter among `reads`, giving the value loaded for
    /// each.
    fn load_params(
        &mut self,
        slots: &Map<Value, Slot>,
        reads: impl IntoIterator<Item = Value>,
        origin: OriginId,
        insts: &mut Vec<Inst>,
    ) -> Map<Value, Value>;
}

impl Demote for Function {
    fn demote_params(&mut self, block: Block, pointer_bits: u32) -> Map<Value, Slot> {
        let params = std::mem::take(&mut self.blocks[block].params);
        let origin = self.blocks[block].origin;
        let slots: Map<Value, Slot> = params
            .iter()
            .map(|&p| {
                let bytes = self.ty(p).bytes(pointer_bits);
                (p, self.slots.push(SlotData { size: bytes, align: bytes, origin }))
            })
            .collect();
        let blocks: Vec<Block> = self.blocks.iter().map(|(b, _)| b).collect();
        for at in blocks {
            let old = std::mem::take(&mut self.blocks[at].insts);
            let mut insts = Vec::with_capacity(old.len());
            for inst in old {
                let (reads, origin): (Vec<Value>, _) =
                    (self.insts[inst].kind.operands().collect(), self.insts[inst].origin);
                let loaded = self.load_params(&slots, reads, origin, &mut insts);
                self.insts[inst].kind.map_values(|v| loaded.get(&v).copied().unwrap_or(v));
                insts.push(inst);
            }
            let reads: Vec<Value> = self.blocks[at].term.operands().collect();
            let term_origin = self.blocks[at].origin;
            let loaded = self.load_params(&slots, reads, term_origin, &mut insts);
            self.blocks[at].term.map_values(|v| loaded.get(&v).copied().unwrap_or(v));
            if let Terminator::Jump(target) = &mut self.blocks[at].term
                && target.block == block
            {
                let args = std::mem::take(&mut target.args);
                for (param, arg) in params.iter().zip(args) {
                    let addr = self.emit(
                        InstKind::SlotAddr(slots[param]),
                        Some(Scalar::Ptr),
                        term_origin,
                        &mut insts,
                    );
                    let addr = addr.expect("an address defines its value");
                    self.emit(
                        InstKind::Store { value: arg, addr, access: Access::Plain },
                        None,
                        term_origin,
                        &mut insts,
                    );
                }
            }
            self.blocks[at].insts = insts;
        }
        slots
    }

    fn load_params(
        &mut self,
        slots: &Map<Value, Slot>,
        reads: impl IntoIterator<Item = Value>,
        origin: OriginId,
        insts: &mut Vec<Inst>,
    ) -> Map<Value, Value> {
        let mut loaded = Map::default();
        for param in reads {
            let Some(&slot) = slots.get(&param) else { continue };
            if loaded.contains_key(&param) {
                continue;
            }
            let ty = self.ty(param);
            let addr = self.emit(InstKind::SlotAddr(slot), Some(Scalar::Ptr), origin, insts);
            let load = InstKind::Load {
                ty,
                addr: addr.expect("an address defines its value"),
                access: Access::Plain,
            };
            loaded.insert(param, self.emit(load, Some(ty), origin, insts).expect("a load defines its value"));
        }
        loaded
    }
}
