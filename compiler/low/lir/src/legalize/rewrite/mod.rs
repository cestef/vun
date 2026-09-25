//! Rewriting one function: each old value is itself or a pair in the new function, each block a
//! block whose wide parameters are pairs, and each operation on a wide integer a sequence over
//! halves (`Halves`).

use std::collections::HashMap;

use vun_index::Idx;
use vun_origin::OriginId;

use super::halves::{Halves, Pair};

mod call;
mod inst;
mod term;
use super::{Helpers, Legalize, Unlegal};
use crate::{Access, Block, Builder, Function, Scalar, Unit, Value};

/// An old value in the new function.
#[derive(Clone, Copy, Debug)]
pub(super) enum Parts {
    One(Value),
    Pair(Pair),
}

pub(super) struct Rewrite<'u> {
    unit: &'u Unit,
    legalize: Legalize,
    wide: Scalar,
    old: &'u Function,
    b: Builder,
    parts: HashMap<Value, Parts>,
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
        wide: Scalar,
        helpers: &'u mut Helpers,
    ) -> Result<Function, Unlegal> {
        let signature = legalize.signature(&old.signature, wide);
        let mut b = Builder::new(old.linkage, signature, old.origin);
        for (_, slot) in old.slots.iter() {
            b.slot(slot.size, slot.align, slot.origin);
        }
        let entry = b.current();
        let mut rw = Self {
            unit,
            legalize,
            wide,
            old,
            b,
            parts: HashMap::new(),
            blocks: Vec::new(),
            result: None,
            helpers,
        };
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
                let parts = if old.ty(p) == wide {
                    Parts::Pair(Pair {
                        lo: received.next().expect("counted"),
                        hi: received.next().expect("counted"),
                    })
                } else {
                    Parts::One(received.next().expect("counted"))
                };
                rw.parts.insert(p, parts);
            }
            // The entry receives the address a wide result is written through last.
            if new == entry {
                rw.result = received.next();
            }
        }
        for (block, data) in old.blocks.iter() {
            rw.b.switch_to(rw.blocks[block.index()]);
            for &inst in &data.insts {
                rw.inst(inst)?;
            }
            rw.terminator(&data.term, data.origin);
        }
        let mut function = rw.b.finish();
        function.stack_max = old.stack_max;
        Ok(function)
    }

    /// The types a value of `ty` becomes.
    fn split(&self, ty: Scalar) -> Vec<Scalar> {
        if ty == self.wide { vec![self.legalize.widest; 2] } else { vec![ty] }
    }

    fn is_wide(&self, v: Value) -> bool {
        self.old.ty(v) == self.wide
    }

    fn one(&self, v: Value) -> Value {
        match self.parts[&v] {
            Parts::One(v) => v,
            Parts::Pair(_) => unreachable!("only a wide value is a pair"),
        }
    }

    fn pair(&self, v: Value) -> Pair {
        match self.parts[&v] {
            Parts::Pair(p) => p,
            Parts::One(_) => unreachable!("a wide value is a pair"),
        }
    }

    /// The new values standing for `v`: itself, or its halves, low first.
    fn flat(&self, v: Value) -> Vec<Value> {
        match self.parts[&v] {
            Parts::One(v) => vec![v],
            Parts::Pair(p) => vec![p.lo, p.hi],
        }
    }

    fn halves(&mut self, at: OriginId) -> Halves<'_> {
        Halves { b: &mut self.b, half: self.legalize.widest, at }
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

    fn load_pair(&mut self, addr: Value, access: Access, at: OriginId) -> Pair {
        let (lo, hi) = self.legalize.offsets();
        let (lo_at, hi_at) = (self.past(addr, lo, at), self.past(addr, hi, at));
        let half = self.legalize.widest;
        Pair { lo: self.b.load(half, lo_at, access, at), hi: self.b.load(half, hi_at, access, at) }
    }

    fn store_pair(&mut self, value: Pair, addr: Value, access: Access, at: OriginId) {
        let (lo, hi) = self.legalize.offsets();
        let (lo_at, hi_at) = (self.past(addr, lo, at), self.past(addr, hi, at));
        self.b.store(value.lo, lo_at, access, at);
        self.b.store(value.hi, hi_at, access, at);
    }
}
