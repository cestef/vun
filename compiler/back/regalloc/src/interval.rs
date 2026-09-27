//! Lifetime intervals: the positions a value, or a scratch register, lives at, as sorted
//! half-open ranges with holes, and the positions it is used at with what each use needs.

use vun_lir::Value;

use crate::machine::{Bank, PReg};
use crate::needs::Need;
use crate::points::Pos;

vun_index::index!(
    pub(crate) struct IntervalId;
);

/// The positions `from..to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Range {
    pub(crate) from: Pos,
    pub(crate) to: Pos,
}

impl Range {
    #[must_use]
    pub(crate) fn covers(self, pos: Pos) -> bool {
        self.from <= pos && pos < self.to
    }
}

/// A position a value is read or written at, and where it must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Use {
    pub(crate) pos: Pos,
    pub(crate) need: Need,
}

impl Use {
    /// Whether the use needs a register, any one or a fixed one it is copied to.
    #[must_use]
    pub(crate) fn needs_reg(self) -> bool {
        self.need == Need::Reg
    }
}

/// Where an interval is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Held {
    /// Not decided yet.
    Unassigned,
    Reg(PReg),
    /// In its value's home in the frame.
    Home,
}

#[derive(Clone, Debug)]
pub(crate) struct Interval {
    /// The value it holds, or `None` for a scratch register.
    pub(crate) value: Option<Value>,
    pub(crate) bank: Bank,
    pub(crate) ranges: Vec<Range>,
    pub(crate) uses: Vec<Use>,
    /// The register it would rather have: where its value is defined or goes next.
    pub(crate) hint: Option<PReg>,
    pub(crate) held: Held,
}

impl Interval {
    #[must_use]
    pub(crate) fn new(value: Option<Value>, bank: Bank) -> Self {
        Self { value, bank, ranges: Vec::new(), uses: Vec::new(), hint: None, held: Held::Unassigned }
    }

    /// # Panics
    /// Panics on an interval without ranges, which is never scanned.
    #[must_use]
    pub(crate) fn start(&self) -> Pos {
        self.ranges.first().expect("a live interval").from
    }

    #[must_use]
    pub(crate) fn end(&self) -> Pos {
        self.ranges.last().expect("a live interval").to
    }

    #[must_use]
    pub(crate) fn covers(&self, pos: Pos) -> bool {
        self.ranges.iter().any(|r| r.covers(pos))
    }

    /// The first position at or after `from` where both are live.
    #[must_use]
    pub(crate) fn intersection(&self, other: &[Range], from: Pos) -> Option<Pos> {
        let (mut i, mut j) = (0, 0);
        while i < self.ranges.len() && j < other.len() {
            let (a, b) = (self.ranges[i], other[j]);
            let lo = a.from.max(b.from).max(from);
            if lo < a.to.min(b.to) {
                return Some(lo);
            }
            if a.to <= b.to { i += 1 } else { j += 1 }
        }
        None
    }

    /// The first use at or after `from` that needs a register.
    #[must_use]
    pub(crate) fn next_reg_use(&self, from: Pos) -> Option<Pos> {
        self.uses.iter().find(|u| u.pos >= from && u.needs_reg()).map(|u| u.pos)
    }

    /// Adds `from..to` while building backwards: it is before the first range or joins it,
    /// possibly past its end where one point reads a value both early and late.
    pub(crate) fn prepend(&mut self, from: Pos, to: Pos) {
        match self.ranges.first_mut() {
            Some(first) if to >= first.from => {
                first.from = first.from.min(from);
                first.to = first.to.max(to);
            }
            _ => self.ranges.insert(0, Range { from, to }),
        }
    }

    /// Starts the interval at its definition `at`, found after the ranges built so far.
    pub(crate) fn define(&mut self, at: Pos) {
        match self.ranges.first_mut() {
            Some(first) => first.from = at,
            // A result nothing reads still takes its register where it is written.
            None => self.ranges.push(Range { from: at, to: at.next() }),
        }
    }

    /// Splits off the part from `at` on, which the returned interval holds.
    ///
    /// # Panics
    /// Panics unless `at` is strictly inside the interval.
    #[must_use]
    pub(crate) fn split(&mut self, at: Pos) -> Self {
        assert!(self.start() < at && at < self.end(), "a split inside the interval");
        let mut rest = Vec::new();
        let mut kept = Vec::new();
        for r in std::mem::take(&mut self.ranges) {
            if r.to <= at {
                kept.push(r);
            } else if r.from >= at {
                rest.push(r);
            } else {
                kept.push(Range { from: r.from, to: at });
                rest.push(Range { from: at, to: r.to });
            }
        }
        self.ranges = kept;
        let cut = self.uses.partition_point(|u| u.pos < at);
        let uses = self.uses.split_off(cut);
        Self {
            value: self.value,
            bank: self.bank,
            ranges: rest,
            uses,
            hint: self.hint,
            held: Held::Unassigned,
        }
    }
}
