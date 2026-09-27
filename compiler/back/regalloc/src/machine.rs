//! An architecture's registers as the allocator sees them: dense indices, grouped in banks.

use std::fmt;

/// A machine register, by the dense index its architecture gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PReg(pub u8);

/// How many registers an architecture may have: a register set is 64 bits.
pub(crate) const REGS: usize = 64;

impl PReg {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for PReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

/// A set of registers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RegSet(u64);

impl RegSet {
    pub const EMPTY: Self = Self(0);

    #[must_use]
    pub fn of(regs: &[PReg]) -> Self {
        regs.iter().fold(Self::EMPTY, |set, &r| set.with(r))
    }

    #[must_use]
    pub const fn with(self, r: PReg) -> Self {
        Self(self.0 | 1 << r.0)
    }

    #[must_use]
    pub const fn contains(self, r: PReg) -> bool {
        self.0 & 1 << r.0 != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = PReg> {
        (0..64_u8).filter(move |&i| self.0 & 1 << i != 0).map(PReg)
    }
}

/// The registers one kind of value is held in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bank {
    General,
    Float,
}

/// What the allocator knows of an architecture's registers.
#[derive(Clone, Debug)]
pub struct Machine {
    /// The registers each bank's values are held in, the preferred first.
    pub general: Vec<PReg>,
    pub float: Vec<PReg>,
    /// The registers a call leaves as they were.
    pub preserved: RegSet,
}

impl Machine {
    /// The registers values of `bank` are held in.
    #[must_use]
    pub fn allocatable(&self, bank: Bank) -> &[PReg] {
        match bank {
            Bank::General => &self.general,
            Bank::Float => &self.float,
        }
    }

    /// The registers a call may overwrite, of those values are held in.
    #[must_use]
    pub fn clobbered(&self) -> RegSet {
        RegSet::of(&self.general).union(RegSet::of(&self.float)).without(self.preserved)
    }
}
