//! From the allocated intervals to the plan: each value's pieces, the moves where a value split
//! inside a block changes place, the copies into and out of fixed registers, and the moves on
//! each edge where a value, or a block parameter's argument, is not where its target expects it;
//! each group of moves a parallel copy, sequenced.

use std::collections::HashMap;

use vun_index::IndexVec;
use vun_lir::Function;

use crate::build::Built;
use crate::interval::{Held, Interval, IntervalId};
use crate::machine::RegSet;
use crate::needs::{Describe, Remade};
use crate::plan::{Allocation, Loc, Move, Piece, Source};
use crate::points::{Points, Pos};

impl Allocation {
    pub(crate) fn resolve(
        f: &Function,
        describe: &impl Describe,
        points: Points,
        built: Built,
        intervals: &IndexVec<IntervalId, Interval>,
    ) -> Self {
        let mut plan = Self {
            pieces: IndexVec::filled(Vec::new(), f.values.len()),
            before: HashMap::new(),
            after: HashMap::new(),
            edges: HashMap::new(),
            temps: HashMap::new(),
            used: RegSet::EMPTY,
            points,
            live: built.live,
        };
        for (_, i) in intervals.iter().filter(|(_, i)| !i.ranges.is_empty()) {
            if let Held::Reg(r) = i.held {
                plan.used = plan.used.with(r);
            }
            if let Some(v) = i.value {
                let at = match i.held {
                    Held::Reg(r) => Loc::Reg(r),
                    Held::Home | Held::Unassigned => Loc::Home(v),
                };
                plan.pieces[v].push(Piece { from: i.start(), to: i.end(), at });
            }
        }
        for (point, id) in built.temps.iter().rev() {
            if let Held::Reg(r) = intervals[*id].held {
                plan.temps.entry(*point).or_default().push(r);
            }
        }
        // Split at a point's read, a value moves before it; split at its write, a value read
        // there keeps its register for the read and is stored home before it, after any reload.
        let (mut reads, mut writes): (HashMap<u32, Vec<Move>>, HashMap<u32, Vec<Move>>) = Default::default();
        for (v, pieces) in plan.pieces.iter_mut() {
            pieces.sort_by_key(|p| p.from);
            for pair in pieces.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if a.at != b.at && !plan.points.at_entry(b.from) {
                    let m = Move { from: Source::At(a.at), to: b.at, value: v };
                    let group = if b.from == b.from.before() { &mut reads } else { &mut writes };
                    group.entry(b.from.point()).or_default().push(m);
                }
            }
        }
        // A value's move out of a place precedes its copy into a fixed register it is read from.
        let mut copies: HashMap<u32, Vec<Move>> = HashMap::new();
        for pinned in &built.copies {
            let from = plan.at(pinned.value, Pos::read(pinned.point)).expect("a value read is live");
            let m = Move { from: Source::At(from), to: Loc::Reg(pinned.at), value: pinned.value };
            copies.entry(pinned.point).or_default().push(m);
        }
        for group in [reads, writes] {
            for (point, moves) in group {
                plan.before.entry(point).or_default().extend(Move::sequence(moves));
            }
        }
        for (point, moves) in copies {
            plan.before.entry(point).or_default().extend(Move::sequence(moves));
        }
        for pinned in &built.landed {
            if let Some(to) = plan.at(pinned.value, Pos::write(pinned.point))
                && to != pinned.at
            {
                let m = Move { from: Source::At(pinned.at), to, value: pinned.value };
                plan.after.entry(pinned.point).or_default().push(m);
            }
        }
        plan.edges(f, describe);
        plan.remake(describe);
        plan
    }

    /// Makes again what values without a home would be reloaded from, and stores none of them.
    fn remake(&mut self, describe: &impl Describe) {
        let held = |v| describe.remade(v) == Some(Remade::Held);
        let groups = self.before.values_mut().chain(self.after.values_mut()).chain(self.edges.values_mut());
        for moves in groups {
            moves.retain(|m| !(m.to == Loc::Home(m.value) && held(m.value)));
            for m in moves.iter_mut().filter(|m| held(m.value)) {
                if m.from == Source::At(Loc::Home(m.value)) {
                    m.from = Source::Remade(m.value);
                }
            }
        }
    }

    fn edges(&mut self, f: &Function, describe: &impl Describe) {
        for &block in &self.points.order {
            let last = self.points.blocks[block].expect("an emitted block").1;
            for target in f.blocks[block].term.targets() {
                let to = target.block;
                let entered = self.points.entered(to);
                let mut moves = Vec::new();
                for v in self.live.into[to].iter() {
                    if let (Some(from), Some(into)) = (self.at(v, Pos::write(last)), self.at(v, entered))
                        && from != into
                    {
                        moves.push(Move { from: Source::At(from), to: into, value: v });
                    }
                }
                for (&param, &arg) in f.blocks[to].params.iter().zip(&target.args) {
                    let Some(into) = self.at(param, entered) else { continue };
                    let from = if describe.recomputed(arg) {
                        Source::Remade(arg)
                    } else {
                        Source::At(self.at(arg, Pos::read(last)).expect("an argument is live at its jump"))
                    };
                    if from != Source::At(into) {
                        moves.push(Move { from, to: into, value: arg });
                    }
                }
                if !moves.is_empty() {
                    self.edges.insert((block, to), Move::sequence(moves));
                }
            }
        }
    }
}

impl Move {
    /// Orders a parallel copy so that nothing is overwritten before every move reading it has
    /// read it; a cycle is broken through `Loc::Temp`.
    pub(crate) fn sequence(mut pending: Vec<Self>) -> Vec<Self> {
        pending.retain(|m| m.from != Source::At(m.to));
        let mut reads: HashMap<Loc, usize> = HashMap::new();
        for m in &pending {
            if let Source::At(at) = m.from {
                *reads.entry(at).or_default() += 1;
            }
        }
        let mut moves = Vec::with_capacity(pending.len());
        while !pending.is_empty() {
            if let Some(i) = pending.iter().position(|m| reads.get(&m.to).is_none_or(|&n| n == 0)) {
                let done = pending.remove(i);
                if let Source::At(at) = done.from {
                    *reads.entry(at).or_default() -= 1;
                }
                moves.push(done);
                continue;
            }
            // Every pending move is on a cycle: save one destination, and read it from there.
            let saved = pending[0].to;
            let value = pending.iter().find(|m| m.from == Source::At(saved)).expect("on a cycle").value;
            moves.push(Self { from: Source::At(saved), to: Loc::Temp, value });
            for m in &mut pending {
                if m.from == Source::At(saved) {
                    m.from = Source::At(Loc::Temp);
                }
            }
            let rerouted = reads.insert(saved, 0).unwrap_or_default();
            *reads.entry(Loc::Temp).or_default() += rerouted;
        }
        moves
    }
}
