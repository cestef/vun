//! A callgrind profile folded into what an optimization is chosen by: the functions that spend
//! instructions and who calls them, and the crates the instructions run in.

use vun_intern::Symbol;

use super::Source;

/// A function, with every instance and recursion level of it summed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Function {
    pub name: Symbol,
    /// Where its code is defined.
    pub source: Source,
    /// Instructions run in it, those of the code inlined into it included.
    pub own: u64,
    /// `own` and what its callees ran for it; none for a function that reaches itself through
    /// others, which has no cost apart from the cycle's.
    pub inclusive: Option<u64>,
    /// How often it was called.
    pub calls: u64,
}

/// A function's calls to another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    /// Indices into [`Profile::functions`].
    pub caller: usize,
    pub callee: usize,
    pub calls: u64,
    /// What the callee ran for these calls, with its own callees; none when they call each other
    /// in a cycle, where no call has a cost of its own.
    pub cost: Option<u64>,
}

/// The instructions run in one source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Share {
    pub source: Source,
    pub instructions: u64,
}

/// What a run executed, the most expensive first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub total: u64,
    /// By own instructions, most first.
    pub functions: Vec<Function>,
    /// By instructions, most first. Inlined code counts where it is written, not in the function
    /// that inlined it.
    pub sources: Vec<Share>,
    pub edges: Vec<Edge>,
}

impl Profile {
    /// The edges into function `callee`, the most expensive first.
    #[must_use]
    pub fn callers(&self, callee: usize) -> Vec<Edge> {
        let mut edges: Vec<Edge> = self.edges.iter().copied().filter(|e| e.callee == callee).collect();
        edges.sort_by_key(|e| std::cmp::Reverse(e.cost));
        edges
    }

    /// The instructions run in each source of a kind, as a table by source.
    #[must_use]
    pub fn share_of(&self, source: Source) -> u64 {
        self.sources.iter().find(|s| s.source == source).map_or(0, |s| s.instructions)
    }
}
