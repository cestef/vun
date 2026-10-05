//! Rewriting one function: each old value is itself, its bits for a promoted `f16` or a float
//! held so, or limbs in the new function, each block a block whose wide parameters are limbs, each operation on a wide
//! integer a sequence over limbs (`Limbwise`) and each on a promoted `f16` one in `f32`
//! (`Float16`).

use vun_hash::{Map, Set};

use vun_codec::{Decode, Encode};
use vun_index::Idx;
use vun_origin::OriginId;

use super::limbs::{Limbs, Limbwise};

mod call;
mod half;
mod held;
mod inst;
mod term;
use super::{Helpers, Legalize, Unlegal};
use vun_lir::analysis::{Cfg, Loops};
use vun_lir::{Access, Block, Builder, Function, Scalar, Unit, Value};

/// An old value in the new function.
#[derive(Clone, Debug, Encode, Decode)]
pub(super) enum Parts {
    One(Value),
    Limbs(Limbs),
}

pub(super) struct Rewrite<'u> {
    unit: &'u Unit,
    legalize: Legalize,
    old: &'u Function,
    b: Builder,
    parts: Map<Value, Parts>,
    blocks: Vec<Block>,
    /// Where a wide result is written: the address the function receives last.
    result: Option<Value>,
    helpers: &'u mut Helpers,
}

impl<'u> Rewrite<'u> {
    pub(super) fn function(
        unit: &'u Unit,
        old: &'u Function,
        legalize: Legalize,
        helpers: &'u mut Helpers,
    ) -> Result<Function, Unlegal> {
        let signature = legalize.signature(&old.signature).map_err(|why| Unlegal { at: old.origin, why })?;
        let mut b = Builder::new(old.linkage, signature, old.origin);
        for (_, slot) in old.slots.iter() {
            b.slot(slot.size, slot.align, slot.origin);
        }
        let entry = b.current();
        let mut rw =
            Self { unit, legalize, old, b, parts: Map::default(), blocks: Vec::new(), result: None, helpers };
        for (block, data) in old.blocks.iter() {
            let new = if Some(block) == old.entry() {
                entry
            } else {
                let types: Vec<Scalar> = data.params.iter().flat_map(|&p| rw.split(old.ty(p))).collect();
                rw.b.block(&types, data.origin)
            };
            rw.blocks.push(new);
            let mut received = rw.b.params(new).to_vec().into_iter();
            for &p in &data.params {
                let parts = match legalize.limbs(old.ty(p)) {
                    Some(limbs) => Parts::Limbs(Limbs(received.by_ref().take(usize::from(limbs)).collect())),
                    None => Parts::One(received.next().expect("counted")),
                };
                rw.parts.insert(p, parts);
            }
            // The entry receives the address a wide result is written through last.
            if new == entry {
                rw.result = received.next();
            }
        }
        // Definitions are rewritten before their uses: reachable blocks in reverse postorder,
        // which visits a block's dominators first, then the others.
        let loops = Loops::of(old, &Cfg::of(old));
        let reached: Set<Block> = loops.rpo().iter().copied().collect();
        let rest = old.blocks.iter().map(|(b, _)| b).filter(|b| !reached.contains(b));
        for block in loops.rpo().iter().copied().chain(rest) {
            let data = &old.blocks[block];
            rw.b.switch_to(rw.blocks[block.index()]);
            for &inst in &data.insts {
                rw.inst(inst)?;
            }
            rw.terminator(&data.term, data.origin);
        }
        let mut function = rw.b.finish();
        function.stack_max = old.stack_max;
        function.stack_check = old.stack_check;
        Ok(function)
    }

    /// The types a value of `ty` becomes.
    fn split(&self, ty: Scalar) -> Vec<Scalar> {
        match self.legalize.limbs(ty) {
            Some(limbs) => vec![self.legalize.widest; usize::from(limbs)],
            None => vec![self.legalize.scalar(ty)],
        }
    }

    /// How many limbs `v` is, where it is wide.
    fn wide(&self, v: Value) -> Option<usize> {
        self.legalize.limbs(self.old.ty(v)).map(usize::from)
    }

    fn is_wide(&self, v: Value) -> bool {
        self.wide(v).is_some()
    }

    fn one(&self, v: Value) -> Value {
        match self.parts[&v] {
            Parts::One(v) => v,
            Parts::Limbs(_) => unreachable!("only a wide value is limbs"),
        }
    }

    fn limbs(&self, v: Value) -> Limbs {
        match &self.parts[&v] {
            Parts::Limbs(l) => l.clone(),
            Parts::One(_) => unreachable!("a wide value is limbs"),
        }
    }

    /// The new values standing for `v`: itself, or its limbs, low first.
    fn flat(&self, v: Value) -> Vec<Value> {
        match &self.parts[&v] {
            Parts::One(v) => vec![*v],
            Parts::Limbs(l) => l.0.clone(),
        }
    }

    fn limbwise(&mut self, at: OriginId) -> Limbwise<'_> {
        Limbwise { b: &mut self.b, limb: self.legalize.widest, at }
    }

    /// The address `bytes` past `addr`.
    fn past(&mut self, addr: Value, bytes: u64, at: OriginId) -> Value {
        if bytes == 0 {
            return addr;
        }
        let word = Scalar::int(self.unit.pointer_bits).expect("addresses are as wide as an integer");
        let offset = self.b.constant(word, u128::from(bytes), at);
        self.b.offset(addr, offset, at)
    }

    fn load_limbs(&mut self, addr: Value, access: Access, count: usize, at: OriginId) -> Limbs {
        let limb = self.legalize.widest;
        let loaded = (0..count).map(|k| {
            let place = self.past(addr, self.legalize.offset(k, count), at);
            self.b.load(limb, place, access, at)
        });
        Limbs(loaded.collect())
    }

    fn store_limbs(&mut self, value: &Limbs, addr: Value, access: Access, at: OriginId) {
        for (k, &limb) in value.0.iter().enumerate() {
            let place = self.past(addr, self.legalize.offset(k, value.len()), at);
            self.b.store(limb, place, access, at);
        }
    }
}
