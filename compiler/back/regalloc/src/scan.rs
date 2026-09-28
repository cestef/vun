//! The scan (Wimmer and Franz, 2010): intervals in order of their start, each given the register
//! free longest, or split where it stops being free; when none is free, the register whose
//! holders need it latest is taken from them, or the interval itself waits in its home until
//! its next use that needs a register. A register only values made again hold is taken first
//! where they need it later, since giving it up stores nothing.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use vun_index::IndexVec;
use vun_lir::Value;

use crate::build::{Built, Fixed};
use crate::interval::{Held, Interval, IntervalId, Range};
use crate::machine::{Machine, PReg, REGS};
use crate::points::Pos;
use crate::split::Splits;

pub(crate) struct Scan<'a> {
    machine: &'a Machine,
    splits: &'a Splits,
    pub(crate) intervals: IndexVec<IntervalId, Interval>,
    fixed: &'a [Vec<Fixed>],
    unhandled: BinaryHeap<Reverse<(Pos, IntervalId)>>,
    active: Vec<IntervalId>,
    inactive: Vec<IntervalId>,
    /// The intervals of each value, as it is split.
    pieces: IndexVec<Value, Vec<IntervalId>>,
}

impl<'a> Scan<'a> {
    #[must_use]
    pub(crate) fn new(machine: &'a Machine, splits: &'a Splits, built: &'a Built) -> Self {
        let intervals = built.intervals.clone();
        let unhandled = intervals
            .iter()
            .filter(|(_, i)| !i.ranges.is_empty())
            .map(|(id, i)| Reverse((i.start(), id)))
            .collect();
        let pieces =
            built.of_value.iter().map(|(_, id)| id.iter().copied().collect()).collect::<Vec<_>>().into();
        Self {
            machine,
            splits,
            intervals,
            fixed: &built.fixed,
            unhandled,
            active: Vec::new(),
            inactive: Vec::new(),
            pieces,
        }
    }

    /// Allocates every interval, following the hints `hint` gives as they are met.
    pub(crate) fn run(
        mut self,
        hint: impl Fn(&Self, IntervalId) -> Option<PReg>,
    ) -> IndexVec<IntervalId, Interval> {
        while let Some(Reverse((pos, current))) = self.unhandled.pop() {
            self.advance(pos);
            if let Some(r) = hint(&self, current) {
                self.intervals[current].hint = Some(r);
            }
            // A value of a bank without registers lives in its home, which every use of it reads.
            if self.regs(current).is_empty() {
                let at = self.intervals[current].start();
                assert!(
                    self.intervals[current].next_reg_use(at).is_none(),
                    "a register of a bank without one"
                );
                self.intervals[current].held = Held::Home;
            } else if !self.free(current) {
                self.blocked(current);
            }
            if let Held::Reg(_) = self.intervals[current].held {
                self.active.push(current);
            }
        }
        self.intervals
    }

    /// The register `id` holds at `pos`, if it is allocated one there.
    #[must_use]
    pub(crate) fn reg_at(&self, id: IntervalId) -> Option<PReg> {
        match self.intervals[id].held {
            Held::Reg(r) => Some(r),
            _ => None,
        }
    }

    /// Moves intervals ended by `pos` out of the active and inactive ones, and those in a hole
    /// there into the inactive ones.
    fn advance(&mut self, pos: Pos) {
        let intervals = &self.intervals;
        let (mut active, mut inactive) = (Vec::new(), Vec::new());
        for id in self.active.drain(..).chain(self.inactive.drain(..)) {
            let i = &intervals[id];
            if i.end() <= pos {
                continue;
            }
            if i.covers(pos) { active.push(id) } else { inactive.push(id) }
        }
        (self.active, self.inactive) = (active, inactive);
    }

    fn regs(&self, current: IntervalId) -> &'a [PReg] {
        self.machine.allocatable(self.intervals[current].bank)
    }

    /// Where each register stops being free for `current`, from its start.
    fn free_until(&self, current: IntervalId) -> [Pos; REGS] {
        let cur = &self.intervals[current];
        let pos = cur.start();
        let mut until = [Pos::MAX; REGS];
        for &a in &self.active {
            if let Held::Reg(r) = self.intervals[a].held {
                until[r.index()] = pos;
            }
        }
        for &a in &self.inactive {
            if let (Held::Reg(r), Some(at)) =
                (self.intervals[a].held, self.intervals[a].intersection(&cur.ranges, pos))
            {
                until[r.index()] = until[r.index()].min(at);
            }
        }
        for &r in self.regs(current) {
            if let Some(at) = self.fixed_from(r, cur, pos) {
                until[r.index()] = until[r.index()].min(at);
            }
        }
        until
    }

    /// Where `r`'s fixed ranges next meet `cur` from `pos`, but those `cur`'s value may be in.
    fn fixed_from(&self, r: PReg, cur: &Interval, pos: Pos) -> Option<Pos> {
        let fixed = &self.fixed[usize::from(r.0)];
        let foreign: Vec<Range> = fixed.iter().filter(|f| !f.owner.allows(cur)).map(|f| f.range).collect();
        cur.intersection(&foreign, pos)
    }

    /// Gives `current` a register free for all of it, or for a start of it split off; `false`
    /// when none is free at its start.
    fn free(&mut self, current: IntervalId) -> bool {
        let (pos, end, hint) = {
            let cur = &self.intervals[current];
            (cur.start(), cur.end(), cur.hint)
        };
        let until = self.free_until(current);
        let regs = self.regs(current);
        let whole = |r: &&PReg| until[r.index()] >= end;
        let chosen =
            hint.filter(|h| regs.contains(h) && whole(&h)).or_else(|| regs.iter().find(whole).copied());
        if let Some(r) = chosen {
            self.intervals[current].held = Held::Reg(r);
            return true;
        }
        let partial = regs.iter().filter(|r| until[r.index()].before() > pos);
        let Some(&r) = partial.max_by_key(|r| until[r.index()]) else { return false };
        self.intervals[current].held = Held::Reg(r);
        self.split_to_unhandled(current, self.splits.at(pos, until[r.index()]));
        true
    }

    /// Takes a register from the intervals holding it, when they need it after `current` next
    /// does; else `current` waits in its home until it needs one.
    fn blocked(&mut self, current: IntervalId) {
        let pos = self.intervals[current].start();
        // Where each register is next needed by what holds it, where a fixed range takes it, and
        // whether only values made again hold it.
        let (mut used, mut blocked, mut remade) = ([Pos::MAX; REGS], [Pos::MAX; REGS], [true; REGS]);
        for &a in &self.active {
            let i = &self.intervals[a];
            if let Held::Reg(r) = i.held {
                // A scratch register is needed for all of it.
                let at = if i.value.is_none() { pos } else { i.next_reg_use(pos).unwrap_or(Pos::MAX) };
                used[r.index()] = used[r.index()].min(at);
                remade[r.index()] &= i.remade;
            }
        }
        let cur = &self.intervals[current];
        for &a in &self.inactive {
            let i = &self.intervals[a];
            if let (Held::Reg(r), Some(_)) = (i.held, i.intersection(&cur.ranges, pos)) {
                used[r.index()] = used[r.index()].min(i.next_reg_use(pos).unwrap_or(Pos::MAX));
                remade[r.index()] &= i.remade;
            }
        }
        for &r in self.regs(current) {
            if let Some(at) = self.fixed_from(r, cur, pos) {
                blocked[r.index()] = at;
                used[r.index()] = used[r.index()].min(at);
            }
        }
        let first = cur.next_reg_use(pos);
        // Past the point `current` first needs it, so those holding it are split between.
        let cheap = |r: &PReg| {
            remade[r.index()]
                && blocked[r.index()] == Pos::MAX
                && first.is_some_and(|f| used[r.index()] > f.next())
        };
        let &r = self
            .regs(current)
            .iter()
            .max_by_key(|r| (cheap(r), used[r.index()]))
            .expect("a bank with registers");
        let (use_at, block_at) = (used[r.index()], blocked[r.index()]);
        match first {
            None => self.intervals[current].held = Held::Home,
            Some(first) if use_at < first => {
                assert!(first > pos, "more registers needed at one point than the bank has");
                self.intervals[current].held = Held::Home;
                if first > pos {
                    self.split_to_unhandled(current, self.splits.at(pos, first));
                }
            }
            Some(_) => {
                assert!(use_at > pos, "more registers needed at one point than the bank has");
                self.intervals[current].held = Held::Reg(r);
                if block_at < self.intervals[current].end() {
                    self.split_to_unhandled(current, self.splits.at(pos, block_at));
                }
                self.evict(r, current, pos);
            }
        }
    }

    /// Splits the intervals holding `r` where `current` takes it, from `pos`: their rest waits in
    /// their home until it next needs a register.
    fn evict(&mut self, r: PReg, current: IntervalId, pos: Pos) {
        let cur_ranges = self.intervals[current].ranges.clone();
        let holders: Vec<IntervalId> = self
            .active
            .iter()
            .chain(&self.inactive)
            .copied()
            .filter(|&a| self.intervals[a].held == Held::Reg(r))
            .filter(|&a| self.intervals[a].intersection(&cur_ranges, pos).is_some())
            .collect();
        for a in holders {
            // Where `current` is written, a holder read there keeps its register for the read
            // and is stored home before the point, so the split may fall between them.
            let at = self.intervals[a].intersection(&cur_ranges, pos).expect("filtered");
            let rest = if self.intervals[a].start() < at { self.split(a, at) } else { a };
            self.active.retain(|&x| x != rest);
            self.inactive.retain(|&x| x != rest);
            self.spill(rest);
        }
    }

    /// Puts `id` in its home until it next needs a register, where the rest is split off to
    /// be allocated again.
    fn spill(&mut self, id: IntervalId) {
        let start = self.intervals[id].start();
        self.intervals[id].held = Held::Home;
        if let Some(next) = self.intervals[id].next_reg_use(start.next()) {
            self.split_to_unhandled(id, self.splits.at(start, next));
        }
    }

    fn split(&mut self, id: IntervalId, at: Pos) -> IntervalId {
        let rest = self.intervals[id].split(at);
        let value = rest.value;
        let rest = self.intervals.push(rest);
        if let Some(v) = value {
            self.pieces[v].push(rest);
        }
        rest
    }

    /// The register `v` is in at `pos`, as allocated so far.
    #[must_use]
    pub(crate) fn reg_of(&self, v: Value, pos: Pos) -> Option<PReg> {
        let piece = self.pieces[v].iter().find(|&&id| {
            let i = &self.intervals[id];
            i.start() <= pos && pos < i.end()
        })?;
        self.reg_at(*piece)
    }

    fn split_to_unhandled(&mut self, id: IntervalId, at: Pos) {
        let rest = self.split(id, at);
        self.unhandled.push(Reverse((at, rest)));
    }
}
