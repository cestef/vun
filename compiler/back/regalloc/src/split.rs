//! Where an interval is split between two positions: at the start of the block least deep in
//! loops between them, the latest of those, so that moves run outside loops and at block
//! boundaries, where resolution places them; else just before the later position's point.

use vun_lir::analysis::Loops;

use crate::points::{Points, Pos};

pub(crate) struct Splits {
    /// The start of each block, in order, with how many loops hold it.
    starts: Vec<(Pos, usize)>,
}

impl Splits {
    #[must_use]
    pub(crate) fn of(points: &Points, loops: &Loops) -> Self {
        let depth = |block| std::iter::successors(loops.innermost(block), |&h| loops.parent(h)).count();
        Self { starts: points.order.iter().map(|&b| (points.start(b), depth(b))).collect() }
    }

    /// An even position in `min + 1..=max`: the moves of a split run before a point reads.
    ///
    /// # Panics
    /// Panics when there is none: the caller splits only where an even position lies between.
    #[must_use]
    pub(crate) fn at(&self, min: Pos, max: Pos) -> Pos {
        let from = self.starts.partition_point(|&(p, _)| p <= min);
        let to = self.starts.partition_point(|&(p, _)| p <= max);
        let boundary = self.starts[from..to].iter().rev().min_by_key(|&&(_, depth)| depth);
        let at = boundary.map_or(max.before(), |&(p, _)| p);
        assert!(min < at && at <= max, "a split between {min:?} and {max:?}");
        at
    }
}
