//! Values' assignments: where each value is, and which values are recomputed where used rather
//! than kept.

use vun_asm::imm::low64;
use vun_index::IndexVec;
use vun_lir::analysis::Order;
use vun_lir::{Address, Function, InstKind, Slot, Value};

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

/// A value recomputed where it is used, never kept: a constant, or an address known while
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
    pub(crate) recompute: Option<Recompute>,
    /// How many times instructions and terminators read it.
    pub(crate) uses: u32,
}

impl Assignment {
    /// The assignments of every value of `f`: the values recomputed where used found in
    /// `order`, a definition coming before its uses, and every value's uses counted.
    pub(crate) fn all(f: &Function, order: &Order, slots: &IndexVec<Slot, i32>) -> IndexVec<Value, Self> {
        let mut values: IndexVec<Value, Self> =
            f.values.iter().map(|_| Self { home: None, recompute: None, uses: 0 }).collect::<Vec<_>>().into();
        for &block in order.blocks() {
            for &inst in &f.blocks[block].insts {
                let data = &f.insts[inst];
                for v in data.kind.operands() {
                    values[v].uses += 1;
                }
                let Some(out) = data.result else { continue };
                let constant = |v: Value| match values[v].recompute {
                    // An offset is address-wide, and native addresses are 64 bits.
                    Some(Recompute::Const(bits)) => Some(low64(bits).cast_signed()),
                    _ => None,
                };
                values[out].recompute = match data.kind {
                    InstKind::Const { bits, .. } => Some(Recompute::Const(bits)),
                    InstKind::SlotAddr(s) => Some(Recompute::Addr(Place::frame(slots[s]))),
                    InstKind::Addr(a) => Some(Recompute::Addr(Place { anchor: Anchor::Sym(a), disp: 0 })),
                    InstKind::Offset { base, offset } => match (values[base].recompute, constant(offset)) {
                        (Some(Recompute::Addr(place)), Some(bytes)) => {
                            Some(Recompute::Addr(place.plus(bytes)))
                        }
                        _ => None,
                    },
                    _ => None,
                };
            }
            for v in f.blocks[block].term.operands() {
                values[v].uses += 1;
            }
        }
        values
    }
}
