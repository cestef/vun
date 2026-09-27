//! What an instruction set tells about blocks of inline assembly (spec 07, section 6): the entries
//! of its table a block names, by their index in declaration order, and its operands as the
//! assembler takes them. Every backend emitting blocks, native code or GNU C, reads them here.

use vun_inline::{Inst, Line, Mem, Operand, RegRef};

use crate::{Isa, Label, Via};

/// Operands of inline assembly a backend may have no encoding or no register for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsmOperand {
    /// Memory with an index register.
    Indexed,
    /// Memory at a symbol's address past a register.
    SymbolOffRegister,
    /// Memory at a fixed address, with no register or symbol.
    Absolute,
    /// Memory at the address of something another module defines, which only its entry in the
    /// global offset table holds.
    Imported,
    /// A value bound in a register of a class no value is allocated to.
    Unallocated,
    /// Operands the instruction's form does not encode, such as an immediate none of its
    /// encodings holds.
    Unencodable,
}

/// An instruction set's facts about blocks of inline assembly.
pub trait Blocks: Isa {
    /// A general register, of the class values are held in.
    type Reg: Copy + Eq + 'static;
    /// What an operand position takes.
    type Kind: Copy;
    /// An operand as the assembler encodes it, `S` naming what the linker places.
    type Operand<S: Copy>;

    /// The kinds of the positions of the form `inst` was checked against, in order.
    fn kinds<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Self::Kind>;

    /// The general registers that form reads or writes without naming them.
    fn implicit<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Self::Reg>;

    /// The general register `r` is, if it is of that class.
    fn general(r: RegRef) -> Option<Self::Reg>;

    /// The register a binding at a position of `kind` must be in, if one: `named`, or the one
    /// the position takes; `None` is an address's register.
    ///
    /// # Errors
    /// The position takes no general register.
    fn fixed(kind: Option<Self::Kind>, named: Option<Self::Reg>) -> Result<Option<Self::Reg>, AsmOperand>;

    /// Whether memory a block names at a position of `kind` is one the assembler addresses.
    ///
    /// # Errors
    /// What it does not address.
    fn addressable<V, P, S>(mem: &Mem<V, P, S>, kind: Self::Kind) -> Result<(), AsmOperand>;

    /// A label, as a branch's target.
    fn label<S: Copy>(label: Label) -> Self::Operand<S>;

    /// Something the linker places, as a branch's target.
    fn target<S: Copy>(target: S) -> Self::Operand<S>;

    /// A block's operand at a position of `kind` as the assembler takes it, placed where `place`
    /// says.
    ///
    /// # Panics
    /// Panics on memory the assembler does not address ([`Blocks::addressable`]).
    fn operand<V, P, S: Copy>(
        operand: &Operand<V, P, S>,
        kind: Self::Kind,
        place: &mut impl Place<Self, S>,
    ) -> Self::Operand<S>;
}

/// Where a block's operands are, as the code it is emitted into has them.
pub trait Place<I: Blocks + ?Sized, S: Copy> {
    /// The register of the next binding, in the order `Block::bindings` gives them.
    fn bound(&mut self) -> I::Reg;

    /// The operand naming the start of `line`.
    fn line(&self, line: Line) -> I::Operand<S>;

    /// How a page or an address reaches `symbol`.
    fn via(&self, symbol: S) -> Via;
}
