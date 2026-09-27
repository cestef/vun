//! Building the intervals in one backward walk of the blocks (Wimmer and Franz, 2010): a value
//! live out of a block covers it whole, a definition starts its interval, a use extends it back to
//! the block's start; fixed registers and clobbers become fixed ranges, scratch registers
//! intervals of their own.

use vun_index::IndexVec;
use vun_lir::analysis::Cfg;
use vun_lir::{Function, Value};

use crate::interval::{Interval, IntervalId, Range, Use};
use crate::live::Live;
use crate::machine::{PReg, RegSet};
use crate::needs::{Describe, Need, Needs, Out, Read};
use crate::plan::Loc;
use crate::points::{Points, Pos};

/// A position range a register is taken at.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixed {
    pub(crate) range: Range,
    pub(crate) owner: Owner,
}

/// What may still be held in a register where a fixed range takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Nobody,
    /// The value read or written there.
    Value(Value),
    /// The results of the point whose clobber it is, written after it.
    Results(u32),
}

impl Owner {
    /// Whether `interval` may be held in the register the range takes.
    pub(crate) fn allows(self, interval: &Interval) -> bool {
        match self {
            Self::Nobody => false,
            Self::Value(v) => interval.value == Some(v),
            Self::Results(point) => interval.value.is_some() && interval.start() == Pos::write(point),
        }
    }
}

/// A value read in a fixed register, or written to a fixed place, at a point.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pinned<L> {
    pub(crate) point: u32,
    pub(crate) value: Value,
    pub(crate) at: L,
}

pub(crate) struct Built {
    pub(crate) intervals: IndexVec<IntervalId, Interval>,
    /// The interval of each kept value, before any split.
    pub(crate) of_value: IndexVec<Value, Option<IntervalId>>,
    /// The fixed ranges of each register, by index, sorted.
    pub(crate) fixed: Vec<Vec<Fixed>>,
    /// The scratch registers' intervals, by point.
    pub(crate) temps: Vec<(u32, IntervalId)>,
    /// Operands copied into fixed registers before their points, and results moved out of them
    /// after.
    pub(crate) copies: Vec<Pinned<PReg>>,
    pub(crate) landed: Vec<Pinned<Loc>>,
    /// Results that would rather have the register of an operand they overwrite.
    pub(crate) follows: Vec<(IntervalId, Value)>,
    pub(crate) live: Live,
}

impl Built {
    #[must_use]
    pub(crate) fn of(f: &Function, d: &impl Describe, cfg: &Cfg, points: &Points) -> Self {
        let kept = |v: Value| !d.recomputed(v);
        let live = Live::of(f, cfg, &points.order, kept);
        let mut b = Self {
            intervals: IndexVec::new(),
            of_value: IndexVec::filled(None, f.values.len()),
            fixed: vec![Vec::new(); 64],
            temps: Vec::new(),
            copies: Vec::new(),
            landed: Vec::new(),
            follows: Vec::new(),
            live,
        };
        let mut needs = Needs::default();
        for &block in points.order.iter().rev() {
            let (start, end) = (points.start(block), points.end(block));
            for v in Live::out(f, cfg, &b.live.into, block, kept).iter() {
                b.interval(f, d, v).prepend(start, end);
            }
            let (first, last) = points.blocks[block].expect("an emitted block");
            for point in (first..=last).rev() {
                points.needs(f, d, point, &mut needs);
                b.point(f, d, point, start, &needs);
            }
        }
        for interval in b.intervals.iter_mut().map(|(_, i)| i) {
            interval.uses.sort_by_key(|u| u.pos);
        }
        for ranges in &mut b.fixed {
            ranges.sort_by_key(|r| r.range.from);
        }
        b
    }

    /// The interval of `v`, made when first met.
    fn interval(&mut self, f: &Function, d: &impl Describe, v: Value) -> &mut Interval {
        let id = if let Some(id) = self.of_value[v] {
            id
        } else {
            let id = self.intervals.push(Interval::new(Some(v), d.bank(f.ty(v))));
            self.of_value[v] = Some(id);
            id
        };
        &mut self.intervals[id]
    }

    /// Records what the point at `point` needs, in the block starting at `start`.
    fn point(&mut self, f: &Function, d: &impl Describe, point: u32, start: Pos, needs: &Needs) {
        let (read, write) = (Pos::read(point), Pos::write(point));
        let pinned: RegSet = needs.defs.iter().fold(RegSet::EMPTY, |set, &(_, out)| match out {
            Out::Fixed(r) => set.with(r),
            _ => set,
        });
        for &(v, out) in &needs.defs {
            let interval = self.interval(f, d, v);
            interval.define(write);
            let need = match out {
                Out::Reg | Out::Reuse(_) => Need::Reg,
                Out::Fixed(r) => {
                    interval.hint = Some(r);
                    Need::Any
                }
                Out::Home | Out::Any => Need::Any,
            };
            interval.uses.push(Use { pos: write, need });
            let id = self.of_value[v].expect("just made");
            match out {
                Out::Fixed(reg) => {
                    self.take(reg, Range { from: write, to: write.next() }, Owner::Value(v));
                    self.landed.push(Pinned { point, value: v, at: Loc::Reg(reg) });
                }
                Out::Home => self.landed.push(Pinned { point, value: v, at: Loc::Home(v) }),
                Out::Reuse(i) => self.follows.push((id, needs.uses[i].value)),
                Out::Reg | Out::Any => {}
            }
        }
        for r in needs.clobbers.without(pinned).iter() {
            self.take(r, Range { from: write, to: write.next() }, Owner::Results(point));
        }
        // A register fixed for a result is the result's once written.
        for r in needs.reserved.iter() {
            let to = if pinned.contains(r) { write } else { write.next() };
            self.take(r, Range { from: read, to }, Owner::Nobody);
        }
        for &bank in &needs.temps {
            let mut temp = Interval::new(None, bank);
            temp.ranges.push(Range { from: read, to: write.next() });
            temp.uses.push(Use { pos: read, need: Need::Reg });
            let id = self.intervals.push(temp);
            self.temps.push((point, id));
        }
        let reused = needs.defs.iter().find_map(|&(_, out)| match out {
            Out::Reuse(i) => Some(i),
            _ => None,
        });
        for (i, &Read { value: v, need, late }) in needs.uses.iter().enumerate() {
            // Read until the result is written, where another operand's register is overwritten.
            // Read until the result is written: used there, so nothing takes its register then.
            let late = late || reused.is_some_and(|r| r != i);
            let at = if late { write } else { read };
            let interval = self.interval(f, d, v);
            interval.prepend(start, at.next());
            let Need::Fixed(reg) = need else {
                interval.uses.push(Use { pos: at, need });
                continue;
            };
            // Copied into its register there: where it is kept needs nothing of the use.
            interval.hint.get_or_insert(reg);
            interval.uses.push(Use { pos: read, need: Need::Any });
            // Read late, it keeps the register until the result is written.
            let to = if late { write.next() } else { write };
            self.take(reg, Range { from: read, to }, Owner::Value(v));
            self.copies.push(Pinned { point, value: v, at: reg });
        }
    }

    fn take(&mut self, r: PReg, range: Range, owner: Owner) {
        self.fixed[usize::from(r.0)].push(Fixed { range, owner });
    }
}
