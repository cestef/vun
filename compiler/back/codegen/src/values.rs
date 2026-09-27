//! Values' assignments: where each value is, and which values are recomputed where used rather
//! than kept.

use vun_asm::imm::low64;
use vun_index::IndexVec;
use vun_lir::analysis::{Liveness, Order};
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

/// Where one value is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Assignment<R> {
    pub(crate) reg: Option<R>,
    /// Its home's offset from the frame pointer, once allocated.
    pub(crate) home: Option<i32>,
    /// Whether the home holds the value.
    pub(crate) saved: bool,
    /// The uses not compiled yet.
    pub(crate) left: u32,
    /// The callee-saved register held for it from definition to last use.
    pub(crate) fixed: Option<R>,
    pub(crate) recompute: Option<Recompute>,
}

impl<R> Assignment<R> {
    fn new(left: u32) -> Self {
        Self { reg: None, home: None, saved: false, left, fixed: None, recompute: None }
    }
}

/// The assignments of every value of `f`, each value's uses counted by `live`, the values
/// recomputed where used found in `order`, a definition coming before its uses.
pub(crate) fn assign<R: Copy>(
    f: &Function,
    live: &Liveness,
    order: &Order,
    slots: &IndexVec<Slot, i32>,
) -> IndexVec<Value, Assignment<R>> {
    let mut values: IndexVec<Value, Assignment<R>> = f
        .values
        .iter()
        .map(|(v, _)| Assignment::new(live.range(v).map_or(0, |r| r.uses)))
        .collect::<Vec<_>>()
        .into();
    for &block in order.blocks() {
        for &inst in &f.blocks[block].insts {
            let data = &f.insts[inst];
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
                    (Some(Recompute::Addr(place)), Some(bytes)) => Some(Recompute::Addr(place.plus(bytes))),
                    _ => None,
                },
                _ => None,
            };
        }
    }
    values
}
