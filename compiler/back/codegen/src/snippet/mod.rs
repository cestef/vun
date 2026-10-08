//! Snippets (TPDE, section 4): an operation's machine instructions as data, over slots for its
//! operands, its result, scratch and fixed registers, which the snippet encoder fills while
//! compiling. For each instruction it chooses operands the architecture's forms take, first an
//! operand as it is (a constant as an immediate, a value in its home as memory, a known address
//! folded into a memory operand), then in a register; it gives the result a dying operand's
//! register instead of copying it, and frees the fixed registers a snippet names before
//! anything else is placed. Where TPDE derives snippets from C through LLVM, these are written
//! against each architecture's instruction table.

mod encode;
mod needs;

use std::fmt::Debug;

pub use encode::Args;

use crate::values::Place;
use crate::{Arch, Code, Processor};

/// What the snippet encoder needs of an architecture's assembler.
pub trait Emits: Arch {
    type Op: Copy + Eq + Debug;
    type Size: Copy + Eq + Debug;
    type Operand: Copy + Eq + Debug;
    type Cond: Copy + Eq + Debug;

    /// The move whose copy of a dying operand into the result the encoder omits.
    const MOV: Self::Op;
    /// The size a move of a register to itself changes nothing at; narrower ones clear the
    /// bits above.
    const WHOLE: Self::Size;

    fn reg(r: Self::Reg) -> Self::Operand;
    fn imm(value: i64) -> Self::Operand;
    fn cond(cond: Self::Cond) -> Self::Operand;
    /// Memory at `place`, when an operand addresses it.
    fn mem(place: Place) -> Option<Self::Operand>;
    /// Memory at the address in `base`.
    fn at(base: Self::Reg) -> Self::Operand;
    /// Memory `index` bytes past the address `base` is, when an operand addresses that.
    fn indexed(base: Self::Operand, index: Self::Reg) -> Option<Self::Operand>;
    /// The size a step of `width` runs at, in an operation of `size`.
    fn size(width: Width<Self::Size>, size: Self::Size) -> Self::Size;
    /// Whether a form of `op` that `processor` has takes `operands` at `size`.
    fn fits(processor: Processor<Self>, op: Self::Op, size: Self::Size, operands: &[Self::Operand]) -> bool;
    /// The registers `op` reads or writes without naming them, freed before a snippet using it
    /// starts.
    fn implicit(op: Self::Op) -> &'static [Self::Reg];
    fn emit(code: &mut Code<Self>, op: Self::Op, size: Self::Size, operands: &[Self::Operand]);
}

/// The width a step runs at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width<S> {
    /// The operation's.
    Op,
    /// The width the architecture computes narrow values at.
    Word,
    Fixed(S),
}

/// Where a step's operand comes from.
#[derive(Debug, PartialEq, Eq)]
pub enum Slot<A: Emits> {
    /// An operand of the operation, by position.
    In(u8),
    /// Memory at the address an operand of the operation holds.
    At(u8),
    /// Memory at the address an operand holds, a byte index past it that another holds, in a
    /// register: `In(index)` as an address.
    Indexed(u8, u8),
    /// The operation's result, in its register.
    Out,
    /// A scratch register of the snippet, by number, taken when first named.
    Temp(u8),
    /// A register the instruction requires, freed before the snippet starts.
    Fixed(A::Reg),
    /// An operand of the operation as an immediate where the form takes one, else in the
    /// register the instruction requires, freed before the snippet starts.
    Through(A::Reg, u8),
    /// The condition the operation is given.
    Cond,
    /// An operand as it is: a register of another bank, an immediate.
    Is(A::Operand),
}

/// One instruction of a snippet.
#[derive(Debug)]
pub struct Step<A: Emits> {
    pub op: A::Op,
    pub width: Width<A::Size>,
    pub slots: &'static [Slot<A>],
}

impl<A: Emits> Step<A> {
    #[must_use]
    pub const fn new(op: A::Op, width: Width<A::Size>, slots: &'static [Slot<A>]) -> Self {
        Self { op, width, slots }
    }
}

// By hand: a derive would ask the architecture itself to be `Copy`.
impl<A: Emits> Clone for Slot<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Emits> Copy for Slot<A> {}

impl<A: Emits> Clone for Step<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Emits> Copy for Step<A> {}

/// An operation as instructions.
pub type Snippet<A> = &'static [Step<A>];
