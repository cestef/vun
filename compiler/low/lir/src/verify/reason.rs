//! What the verifier finds malformed, and where.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_index::Idx;

use crate::entity::{Block, FuncId, Inst, Slot, Value};
use crate::inst::Address;
use crate::ty::Scalar;

/// Where in a unit something is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Entity {
    Func(FuncId),
    Block(FuncId, Block),
    Inst(FuncId, Inst),
    Slot(FuncId, Slot),
    Data(usize),
    Import(usize),
}

/// Why; see [`Unit::verify`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Reason {
    NoOrigin,
    /// A function without blocks.
    Empty,
    /// The entry block's parameters are not the signature's values.
    EntryParams,
    UnknownBlock(Block),
    UnknownValue(Value),
    /// A value whose definition does not dominate a use of it.
    NotDominated(Value),
    /// An instruction in no block, or in two.
    Placement(Inst),
    /// An address or callee naming nothing in the unit.
    UnknownAddress(Address),
    /// A jump passing values that are not the target's parameters in number or type.
    Arguments(Block),
    /// Two jumps of one terminator to a block, with different arguments: moves on an edge belong
    /// to its pair of blocks (spec 13, section 8.1).
    Edge(Block),
    /// An operand, or the value defined, of a type the instruction does not accept.
    Type {
        expected: Scalar,
        found: Scalar,
    },
    /// An operation on a type it does not apply to, such as `fadd` of integers.
    Operation(Scalar),
    /// A constant with bits past its type.
    Constant,
    /// A call whose arguments are not the callee's values in number.
    Arity,
    /// A return whose value does not match the signature's result.
    Return,
    /// A signature passing an integer without how it extends, or anything else with it.
    Passing,
    /// A signature marking variadic arguments outside the C convention or past its parameters,
    /// or a function with a body taking them.
    Variadic,
    /// An offset leaving the object its base points into, `at` bytes into an object of `size`.
    OutOfBounds {
        size: u64,
        at: i128,
    },
    /// A value whose definition does not name it back: the instruction giving it, or the block
    /// parameter at its position.
    Definition(Value),
    /// A jump to the entry block, whose parameters only a call passes.
    EntryTargeted,
    /// A switch with two cases of one value.
    DuplicateCase(u128),
    /// An alignment that is not a power of two.
    Alignment(u64),
    /// An assembly block that does not return, not ending its block as unreachable.
    PastNever,
    /// In a naked function, which has no frame: a slot, a return, or an instruction other than
    /// an assembly block without bindings.
    Naked,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Malformed {
    pub at: Entity,
    pub reason: Reason,
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Func(id) => write!(f, "{id}"),
            Self::Block(id, block) => write!(f, "{block} of {id}"),
            Self::Inst(id, inst) => write!(f, "instruction {} of {id}", inst.index()),
            Self::Slot(id, slot) => write!(f, "{slot} of {id}"),
            Self::Data(i) => write!(f, "d{i}"),
            Self::Import(i) => write!(f, "i{i}"),
        }
    }
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.at, self.reason)
    }
}

impl std::error::Error for Malformed {}
