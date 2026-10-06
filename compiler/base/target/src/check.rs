//! What a program's unsafe code may be checked for at run time (spec 07, section 7).

use vun_codec::{Decode, Encode};

use crate::Chosen;

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
pub type Checks = Chosen<Check>;
