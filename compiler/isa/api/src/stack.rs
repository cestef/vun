//! What an instruction of a stack machine does to its stack and to the blocks around it.

use vun_inline::ClassId;

/// A value on a stack machine's stack: of the class of locals that holds it, or of any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Class(ClassId),
    Any,
}

/// A kind of block of a stack machine's code, by how a branch reaches it and what follows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nest {
    /// A branch to it goes to its end.
    Plain,
    /// A branch to it goes to its start, so carries nothing.
    Loop,
    /// A plain block that an `else` may follow.
    If,
}

/// What an instruction of a stack machine does to the nesting of its code, which is blocks, each
/// giving at most one value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    None,
    /// Opens a block of the kind, giving `yields` where it gives a value.
    Open {
        nest: Nest,
        yields: Option<Slot>,
    },
    /// Ends the first arm of an `if` and opens the second.
    Reopen,
    /// Ends the innermost block.
    Close,
    /// Goes to the block `depth` blocks out of the innermost, which `always` does, so that what
    /// follows is not reached; otherwise only when a value it pops says so.
    Branch {
        depth: u32,
        always: bool,
    },
    /// Goes to one of the blocks at these depths, the last the default.
    Table(Vec<u32>),
    /// Calls the function its operand names, which pops and pushes what the function's type says:
    /// the checker of the instruction set fills them in.
    Call,
}

/// What an instruction of a stack machine does to its stack: the values it pops, the last on
/// top, and the ones it pushes, and to the blocks around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub pops: Vec<Slot>,
    pub pushes: Vec<Slot>,
    pub scope: Scope,
}
