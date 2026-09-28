//! How a function reaches its slots, and the scalars each slot splits into: its tiles.

use std::collections::{BTreeMap, BTreeSet};

use vun_hash::{Map, Set};

use vun_lir::entity::{Slot, Value};
use vun_lir::func::Function;
use vun_lir::inst::{Access, InstKind};
use vun_lir::ty::Scalar;

/// The most tiles a slot splits into: an array wider is left in memory.
const MOST: usize = 16;

/// Where an address points: a slot, and bytes past its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Place {
    pub slot: Slot,
    pub offset: u64,
}

/// The scalars of a slot, by offset, covering it.
pub(super) type Tiles = BTreeMap<u64, Scalar>;

/// A copy with a slot on one side at least.
struct Copied {
    to: Option<Place>,
    from: Option<Place>,
    bytes: u64,
}

/// How a function reaches its slots: the addresses into them, the scalars read and written
/// there, the copies to and from them, and the slots reached otherwise.
pub(super) struct Reach {
    pub places: Map<Value, Place>,
    escaped: Set<Slot>,
    scalars: Map<Slot, Vec<(u64, Scalar)>>,
    copies: Vec<Copied>,
}

impl Reach {
    pub(super) fn of(f: &Function) -> Self {
        let mut reach = Self {
            places: Map::default(),
            escaped: Set::default(),
            scalars: Map::default(),
            copies: Vec::new(),
        };
        let insts = || f.blocks.iter().flat_map(|(_, b)| &b.insts).map(|&i| &f.insts[i]);
        for inst in insts() {
            let place = match inst.kind {
                InstKind::SlotAddr(slot) => Some(Place { slot, offset: 0 }),
                InstKind::Offset { base, offset } => reach.places.get(&base).copied().and_then(|at| {
                    let (_, bits) = f.constant(offset)?;
                    Some(Place { slot: at.slot, offset: at.offset.checked_add(u64::try_from(bits).ok()?)? })
                }),
                _ => None,
            };
            if let (Some(place), Some(result)) = (place, inst.result) {
                reach.places.insert(result, place);
            }
        }
        for inst in insts() {
            match &inst.kind {
                InstKind::Load { ty, addr, access: Access::Plain } if reach.places.contains_key(addr) => {
                    reach.scalar(reach.places[addr], *ty);
                }
                InstKind::Store { value, addr, access: Access::Plain } if reach.places.contains_key(addr) => {
                    reach.scalar(reach.places[addr], f.ty(*value));
                    reach.escape(*value);
                }
                InstKind::Copy { to, from, bytes } => {
                    let (to, from) = (reach.places.get(to).copied(), reach.places.get(from).copied());
                    if to.is_some() || from.is_some() {
                        match (to, from) {
                            (Some(a), Some(b)) if a.slot == b.slot => {
                                reach.escaped.insert(a.slot);
                            }
                            _ => reach.copies.push(Copied { to, from, bytes: *bytes }),
                        }
                    }
                }
                InstKind::Offset { base, offset } if f.constant(*offset).is_some() => {
                    reach.escape_unless_placed(*base, inst.result);
                }
                kind => {
                    for value in kind.operands() {
                        reach.escape(value);
                    }
                }
            }
        }
        let terms: Vec<Value> = f.blocks.iter().flat_map(|(_, b)| b.term.operands()).collect();
        for value in terms {
            reach.escape(value);
        }
        reach
    }

    fn scalar(&mut self, at: Place, ty: Scalar) {
        self.scalars.entry(at.slot).or_default().push((at.offset, ty));
    }

    /// The slot `value` addresses, if it does, is reached otherwise than by its tiles.
    fn escape(&mut self, value: Value) {
        if let Some(place) = self.places.get(&value) {
            self.escaped.insert(place.slot);
        }
    }

    /// An offset from `base` not placed, being out of any slot's reach, lets its slot escape.
    fn escape_unless_placed(&mut self, base: Value, result: Option<Value>) {
        if result.is_none_or(|r| !self.places.contains_key(&r)) {
            self.escape(base);
        }
    }

    /// The tiles of every slot split: reached only by plain scalars and copies, in bounds, each
    /// scalar exactly a tile of one type, and every copy between two slots split tiling both
    /// alike.
    pub(super) fn tilings(&self, f: &Function, pointer_bits: u32) -> Map<Slot, Tiles> {
        let slots = self
            .scalars
            .keys()
            .copied()
            .chain(self.copies.iter().flat_map(|c| [c.to, c.from]).flatten().map(|p| p.slot));
        let mut tilings: Map<Slot, Tiles> = Map::default();
        for slot in slots.collect::<Set<_>>() {
            if !self.escaped.contains(&slot)
                && let Some(tiles) = self.tiles(f, slot, pointer_bits)
            {
                tilings.insert(slot, tiles);
            }
        }
        loop {
            let disagree: Vec<Slot> = self
                .copies
                .iter()
                .filter_map(|c| Some((c.to?, c.from?, c.bytes)))
                .filter(|&(to, from, bytes)| match (tilings.get(&to.slot), tilings.get(&from.slot)) {
                    (Some(a), Some(b)) => !Self::alike(a, to.offset, b, from.offset, bytes, pointer_bits),
                    _ => false,
                })
                .flat_map(|(to, from, _)| [to.slot, from.slot])
                .collect();
            if disagree.is_empty() {
                return tilings;
            }
            for slot in disagree {
                tilings.remove(&slot);
            }
        }
    }

    /// Whether `a` from `at` and `b` from `bt` have tiles of the same sizes over `bytes` bytes:
    /// a copy between them moves each tile whole, whatever its scalar.
    fn alike(a: &Tiles, at: u64, b: &Tiles, bt: u64, bytes: u64, pointer_bits: u32) -> bool {
        let a = a.range(at..at + bytes).map(|(&o, &ty)| (o - at, ty.bytes(pointer_bits)));
        a.eq(b.range(bt..bt + bytes).map(|(&o, &ty)| (o - bt, ty.bytes(pointer_bits))))
    }

    /// The tiles of `slot`: bounds where its scalars and copies start and end, and where those
    /// of the slots it is copied to or from do; each stretch between split into the widest
    /// aligned integers, or the scalar read or written there or in a slot copied there. A slot already one scalar, never
    /// copied, is left as it is.
    fn tiles(&self, f: &Function, slot: Slot, pointer_bits: u32) -> Option<Tiles> {
        let data = f.slots[slot];
        let own = self.scalars.get(&slot).map_or(&[][..], Vec::as_slice);
        let mut bounds: BTreeSet<u64> = BTreeSet::from([0, data.size]);
        for &(offset, ty) in own {
            bounds.extend([offset, offset + ty.bytes(pointer_bits)]);
        }
        // The scalars of the slots copied to or from, where they fall in this one.
        let mut copied: Map<u64, Scalar> = Map::default();
        for copy in &self.copies {
            let (here, there) = match (copy.to, copy.from) {
                (Some(p), other) if p.slot == slot => (p, other),
                (other, Some(p)) if p.slot == slot => (p, other),
                _ => continue,
            };
            bounds.extend([here.offset, here.offset + copy.bytes]);
            let theirs = there.and_then(|t| Some((t, self.scalars.get(&t.slot)?)));
            if let Some((there, scalars)) = theirs {
                let inside =
                    scalars.iter().filter(|(o, _)| (there.offset..there.offset + copy.bytes).contains(o));
                for &(offset, ty) in inside {
                    let start = here.offset + offset - there.offset;
                    let end = start + ty.bytes(pointer_bits);
                    bounds.extend([start, end.min(here.offset + copy.bytes)]);
                    if end <= here.offset + copy.bytes {
                        copied.insert(start, ty);
                    }
                }
            }
        }
        if bounds.last() != Some(&data.size) {
            return None;
        }
        let mut typed: Map<u64, Scalar> = Map::default();
        for &(offset, ty) in own {
            if typed.insert(offset, ty).is_some_and(|seen| seen != ty) {
                return None;
            }
        }
        let mut tiles = Tiles::new();
        let bounds: Vec<u64> = bounds.into_iter().collect();
        for stretch in bounds.windows(2) {
            let mut at = stretch[0];
            while at < stretch[1] {
                let ty = match typed.get(&at) {
                    Some(&ty) if at + ty.bytes(pointer_bits) == stretch[1] => ty,
                    Some(_) => return None,
                    None => match copied.get(&at) {
                        Some(&ty) if at + ty.bytes(pointer_bits) == stretch[1] => ty,
                        _ => {
                            let fits = |&n: &u64| n <= data.align && at % n == 0 && at + n <= stretch[1];
                            let bytes = [8, 4, 2, 1].into_iter().find(fits)?;
                            Scalar::int(u32::try_from(bytes * 8).ok()?)?
                        }
                    },
                };
                tiles.insert(at, ty);
                at += ty.bytes(pointer_bits);
            }
        }
        let copied = self.copies.iter().any(|c| [c.to, c.from].into_iter().flatten().any(|p| p.slot == slot));
        if tiles.len() == 1 && !copied {
            // One scalar filling the slot: promotion takes it as it is.
            return None;
        }
        (tiles.len() <= MOST).then_some(tiles)
    }
}
