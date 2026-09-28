//! Scalarizing slots (spec 13, section 3): a local aggregate lowering keeps in a slot, reached
//! only by plain loads and stores at constant offsets and by copies, is its fields in disguise.
//! The slot is cut into tiles, one per scalar read or written in it and the widest aligned
//! integers between, agreeing with the slots it is copied to or from; each copy touching it
//! becomes a load and a store per tile, and each tile a slot of its own, which promotion then
//! makes a value. A struct built field by field and read back, or passed through a small
//! function inlined, never reaches memory.

#[cfg(test)]
mod tests;
mod tiles;

use vun_hash::Map;
use vun_origin::OriginId;

use tiles::{Place, Reach, Tiles};
use vun_lir::entity::{Inst, Slot, Value};
use vun_lir::func::{Function, SlotData};
use vun_lir::inst::{Access, InstKind};
use vun_lir::ty::Scalar;

pub trait Scalarize {
    /// Splits every slot reached only by plain scalars at constant offsets and by copies into
    /// its tiles (see the module), returning how many were.
    fn scalarize(&mut self, pointer_bits: u32) -> usize;
}

impl Scalarize for Function {
    fn scalarize(&mut self, pointer_bits: u32) -> usize {
        if self.slots.is_empty() {
            return 0;
        }
        let reach = Reach::of(self);
        let tilings = reach.tilings(self, pointer_bits);
        if tilings.is_empty() {
            return 0;
        }
        let mut split = Split { reach: &reach, tilings: &tilings, pointer_bits, slots: Map::default() };
        let blocks: Vec<_> = self.blocks.iter().map(|(b, _)| b).collect();
        for block in blocks {
            let old = std::mem::take(&mut self.blocks[block].insts);
            let mut insts = Vec::with_capacity(old.len());
            for inst in old {
                split.rewrite(self, inst, &mut insts);
            }
            self.blocks[block].insts = insts;
        }
        // The copies expanded are in no block now.
        self.compact();
        vun_trace::tracing::trace!(count = tilings.len(), "slots scalarized");
        tilings.len()
    }
}

struct Split<'r> {
    reach: &'r Reach,
    tilings: &'r Map<Slot, Tiles>,
    pointer_bits: u32,
    /// The slot of each tile made so far, by the slot split and its offset.
    slots: Map<(Slot, u64), Slot>,
}

impl Split<'_> {
    /// Appends `inst` to `insts`, reaching tiles rather than the slots split.
    fn rewrite(&mut self, f: &mut Function, inst: Inst, insts: &mut Vec<Inst>) {
        let origin = f.insts[inst].origin;
        match f.insts[inst].kind.clone() {
            InstKind::Load { ty, addr, access: Access::Plain } if self.tiled(addr).is_some() => {
                let addr = self.tile(f, self.tiled(addr).expect("tiled"), insts, origin);
                f.insts[inst].kind = InstKind::Load { ty, addr, access: Access::Plain };
            }
            InstKind::Store { value, addr, access: Access::Plain } if self.tiled(addr).is_some() => {
                let addr = self.tile(f, self.tiled(addr).expect("tiled"), insts, origin);
                f.insts[inst].kind = InstKind::Store { value, addr, access: Access::Plain };
            }
            InstKind::Copy { to, from, bytes } => {
                let Some((at, tiles)) = [to, from].into_iter().find_map(|v| {
                    let place = self.tiled(v)?;
                    Some((place, &self.tilings[&place.slot]))
                }) else {
                    insts.push(inst);
                    return;
                };
                for (&offset, &ty) in tiles.range(at.offset..at.offset + bytes) {
                    let past = offset - at.offset;
                    let from = self.address(f, from, past, insts, origin);
                    let to = self.address(f, to, past, insts, origin);
                    let value = f.emit(
                        InstKind::Load { ty, addr: from, access: Access::Plain },
                        Some(ty),
                        origin,
                        insts,
                    );
                    let store = InstKind::Store {
                        value: value.expect("a load defines its value"),
                        addr: to,
                        access: Access::Plain,
                    };
                    f.emit(store, None, origin, insts);
                }
                return;
            }
            _ => {}
        }
        insts.push(inst);
    }

    /// Where `addr` points, if in a slot split.
    fn tiled(&self, addr: Value) -> Option<Place> {
        self.reach.places.get(&addr).copied().filter(|p| self.tilings.contains_key(&p.slot))
    }

    /// The address `past` bytes after `addr`: a tile's where `addr` is in a slot split.
    fn address(
        &mut self,
        f: &mut Function,
        addr: Value,
        past: u64,
        insts: &mut Vec<Inst>,
        origin: OriginId,
    ) -> Value {
        if let Some(place) = self.tiled(addr) {
            return self.tile(f, Place { slot: place.slot, offset: place.offset + past }, insts, origin);
        }
        if past == 0 {
            return addr;
        }
        let word = Scalar::int(self.pointer_bits).expect("addresses are as wide as an integer");
        let offset = f.emit(InstKind::Const { ty: word, bits: u128::from(past) }, Some(word), origin, insts);
        let offset = offset.expect("a constant defines its value");
        f.emit(InstKind::Offset { base: addr, offset }, Some(Scalar::Ptr), origin, insts)
            .expect("an offset defines its value")
    }

    /// The address of the tile at `at`, its slot made the first time.
    fn tile(&mut self, f: &mut Function, at: Place, insts: &mut Vec<Inst>, origin: OriginId) -> Value {
        let ty = self.tilings[&at.slot][&at.offset];
        let whole = f.slots[at.slot];
        let slot = *self.slots.entry((at.slot, at.offset)).or_insert_with(|| {
            let bytes = ty.bytes(self.pointer_bits);
            f.slots.push(SlotData { size: bytes, align: bytes.min(whole.align), origin: whole.origin })
        });
        f.emit(InstKind::SlotAddr(slot), Some(Scalar::Ptr), origin, insts)
            .expect("an address defines its value")
    }
}
