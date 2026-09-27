//! Values' assignments: where each value is, and which values are recomputed where used rather
//! than kept.

use std::iter;

use vun_asm::imm::low64;
use vun_index::{BitSet, IndexVec};
use vun_lir::analysis::{Loops, Order};
use vun_lir::{Address, Function, InstKind, Slot, Value};
use vun_regalloc::Remade;

use crate::Arch;

/// What an address in a function is relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// The frame pointer.
    Frame,
    /// Something the linker places.
    Sym(Address),
}

/// An address known while compiling: `disp` bytes past its anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub anchor: Anchor,
    pub disp: i64,
}

impl Place {
    #[must_use]
    pub const fn frame(offset: i32) -> Self {
        Self { anchor: Anchor::Frame, disp: offset as i64 }
    }

    #[must_use]
    pub const fn plus(self, bytes: i64) -> Self {
        Self { disp: self.disp.wrapping_add(bytes), ..self }
    }
}

/// A value made anywhere rather than computed once: a constant, or an address known while
/// compiling (a slot's, a datum's, a function's, possibly at a constant offset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recompute {
    /// The constant's bits.
    Const(u128),
    Addr(Place),
}

/// What the framework keeps of one value; where it is, the plan says.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Assignment {
    /// Its home's offset from the frame pointer, once allocated.
    pub(crate) home: Option<i32>,
    /// How it is made anywhere, and whether where used or held.
    pub(crate) made: Option<(Recompute, Remade)>,
    /// How many times instructions and terminators read it.
    pub(crate) uses: u32,
}

impl Assignment {
    /// The assignments of every value of `f`: the values made anywhere found in `order`, a
    /// definition coming before its uses, and every value's uses counted. One is held where `A`
    /// does not take it in place and a use is in a loop its definition is outside of, as a
    /// constant hoisted out of a loop is; else it is made where used.
    pub(crate) fn all<A: Arch>(
        f: &Function,
        loops: &Loops,
        order: &Order,
        slots: &IndexVec<Slot, i32>,
        pointer_bits: u32,
    ) -> IndexVec<Value, Self> {
        let mut values: IndexVec<Value, Self> =
            f.values.iter().map(|_| Self { home: None, made: None, uses: 0 }).collect::<Vec<_>>().into();
        let depth = |block| iter::successors(loops.innermost(block), |&h| loops.parent(h)).count();
        let (mut defined, mut deeper) = (IndexVec::filled(0, f.values.len()), BitSet::new());
        for &block in order.blocks() {
            let here = depth(block);
            let mut read =
                |values: &mut IndexVec<Value, Self>, defined: &IndexVec<Value, usize>, v: Value| {
                    values[v].uses += 1;
                    if here > defined[v] {
                        deeper.insert(v);
                    }
                };
            for &inst in &f.blocks[block].insts {
                let data = &f.insts[inst];
                for v in data.kind.operands() {
                    read(&mut values, &defined, v);
                }
                let Some(out) = data.result else { continue };
                defined[out] = here;
                let constant = |v: Value| match values[v].made {
                    // An offset is address-wide, and native addresses are 64 bits.
                    Some((Recompute::Const(bits), _)) => Some(low64(bits).cast_signed()),
                    _ => None,
                };
                let made = match data.kind {
                    InstKind::Const { bits, .. } => Some(Recompute::Const(bits)),
                    InstKind::SlotAddr(s) => Some(Recompute::Addr(Place::frame(slots[s]))),
                    InstKind::Addr(a) => Some(Recompute::Addr(Place { anchor: Anchor::Sym(a), disp: 0 })),
                    InstKind::Offset { base, offset } => match (values[base].made, constant(offset)) {
                        (Some((Recompute::Addr(place), _)), Some(bytes)) => {
                            Some(Recompute::Addr(place.plus(bytes)))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                values[out].made = made.map(|what| (what, Remade::Used));
            }
            for v in f.blocks[block].term.operands() {
                read(&mut values, &defined, v);
            }
        }
        for (v, a) in values.iter_mut() {
            if let Some((what, remade)) = &mut a.made
                && deeper.contains(v)
                && !A::in_place(*what, f.ty(v).bits(pointer_bits))
            {
                *remade = Remade::Held;
            }
        }
        values
    }
}
