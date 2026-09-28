//! The registers of one class a call takes, in the order its values take them.

use crate::Pair;

/// How values take the registers of a bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Span {
    /// A register each, whatever its width.
    One,
    /// A register per word of these bytes.
    Words(u64),
    /// Registers of these bytes overlapping wider ones: a wider value takes an aligned run of
    /// them, named by its first, the lowest free.
    Overlapping(u64),
}

/// The registers of one class, and which are taken.
pub(crate) struct Bank<R: 'static> {
    regs: &'static [R],
    span: Span,
    /// A bit per register taken or closed.
    taken: u64,
    /// The register after the last taken, where the next value starts but under
    /// [`Span::Overlapping`].
    next: usize,
    /// How many registers values took.
    used: usize,
}

impl Span {
    /// How many registers a value of `bytes` takes.
    pub(crate) fn units(self, bytes: u64) -> usize {
        match self {
            Self::One => 1,
            Self::Words(unit) | Self::Overlapping(unit) => {
                usize::try_from(bytes.div_ceil(unit)).expect("a register count")
            }
        }
    }

    /// The alignment of the first register of a run whose first value takes `units`, for values
    /// aligned to `align` in memory.
    fn align(self, units: usize, align: u64, pair: Pair) -> usize {
        match self {
            Self::Words(word) if pair == Pair::Even && align >= 2 * word => 2,
            Self::One | Self::Words(_) => 1,
            Self::Overlapping(_) => units,
        }
    }
}

/// The bits of `count` registers from `start`.
fn bits(start: usize, count: usize) -> u64 {
    let count = u32::try_from(count).expect("at most 64 registers");
    u64::MAX.checked_shr(u64::BITS - count).unwrap_or(0) << start
}

impl<R: Copy> Bank<R> {
    pub(crate) fn new(regs: &'static [R], span: Span) -> Self {
        assert!(regs.len() <= 64, "a bank of at most 64 registers");
        Self { regs, span, taken: 0, next: 0, used: 0 }
    }

    pub(crate) fn span(&self) -> Span {
        self.span
    }

    /// The first register of a run of values taking `units` registers in all, the first `first`,
    /// aligned to `align` in memory: the next free, or the lowest free for overlapping ones.
    pub(crate) fn find(&self, units: usize, first: usize, align: u64, pair: Pair) -> Option<usize> {
        let step = self.span.align(first, align, pair);
        let fits = |start: usize| start + units <= self.regs.len() && self.taken & bits(start, units) == 0;
        match self.span {
            Span::Overlapping(_) => (0..self.regs.len()).step_by(step).find(|&start| fits(start)),
            Span::One | Span::Words(_) => Some(self.next.next_multiple_of(step)).filter(|&start| fits(start)),
        }
    }

    /// Takes `units` registers from `start`.
    pub(crate) fn claim(&mut self, start: usize, units: usize) {
        self.taken |= bits(start, units);
        self.next = self.next.max(start + units);
        self.used += units;
    }

    /// The register `k` of the bank.
    pub(crate) fn reg(&self, k: usize) -> R {
        self.regs[k]
    }

    /// How many registers are left from register `start` on.
    pub(crate) fn left(&self, start: usize) -> usize {
        self.regs.len().saturating_sub(start)
    }

    /// Leaves no register to later values.
    pub(crate) fn close(&mut self) {
        self.next = self.regs.len();
        self.taken = bits(0, self.regs.len());
    }

    pub(crate) fn used(&self) -> usize {
        self.used
    }

    /// Where the next value aligned to `align` in memory starts, but under
    /// [`Span::Overlapping`].
    pub(crate) fn next(&self, align: u64, pair: Pair) -> usize {
        self.next.next_multiple_of(self.span.align(1, align, pair))
    }
}
