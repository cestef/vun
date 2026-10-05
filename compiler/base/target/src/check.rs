//! What a program's unsafe code may be checked for at run time (spec 07, section 7).

use vun_codec::{Decode, Encode};

/// `lang.check.Kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum Check {
    /// Reads and writes through raw pointers.
    Pointers,
    /// What allocators hand out and are given back.
    Allocations,
    /// The memory assembly blocks write, and the stack pointer they return with.
    Asm,
    /// That each function's frame lies within the stack the runtime gives.
    Stack,
}

/// The checks a program is built with, each once, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Checks(Vec<Check>);

impl Checks {
    #[must_use]
    pub fn of(kinds: impl IntoIterator<Item = Check>) -> Self {
        let mut kinds: Vec<Check> = kinds.into_iter().collect();
        kinds.sort_unstable();
        kinds.dedup();
        Self(kinds)
    }

    #[must_use]
    pub fn contains(&self, kind: Check) -> bool {
        self.0.contains(&kind)
    }

    pub fn iter(&self) -> impl Iterator<Item = Check> {
        self.0.iter().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
