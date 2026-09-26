//! The single-pass code generation framework (spec 13, section 8.3), after TPDE: what does not
//! depend on the architecture. LIR is read directly with its shared analyses; blocks are
//! compiled in `Order`, instructions in program order, each selected, given registers and
//! encoded at once into a `vun_asm::Buffer`.
//!
//! An architecture brings its registers and the framework's own moves ([`Arch`]), its
//! instruction compilers ([`Isel`]), and, for instructions described as data, the operands and
//! forms its assembler takes ([`snippet::Emits`]). The framework brings the rest: every value's
//! assignment (its register, its home in the frame, whether the home holds it, its remaining
//! uses), register allocation with eviction, values recomputed where used (constants and
//! addresses), callee-saved registers for values live across loops, the frame, jumps filling
//! their targets' parameters, compare and branch fusion, and the snippet encoder.

mod cx;
mod frame;
mod pieces;
mod regs;
pub mod snippet;
mod values;

pub use cx::{Compiled, Cx, Fused, compile};
pub use frame::Frame;
pub use pieces::pieces;
pub use values::{Anchor, Place, Recompute};

use vun_backend::Lack;
use vun_lir::{Address, Block, Cond, Function, Inst};

/// A function's code as it is emitted.
pub type Code<A> = vun_asm::Buffer<<A as Arch>::Isa, Address>;

/// What the framework knows of an architecture: the registers values are held in, and the moves
/// it makes itself.
pub trait Arch: Sized + 'static {
    type Isa: vun_asm::Isa;
    /// A register of the bank values are held in.
    type Reg: Copy + Eq + std::fmt::Debug + 'static;
    /// What the instruction compilers keep for one function.
    type State;

    /// How many registers the bank has, numbered densely by [`Arch::index`].
    const REGS: usize;
    /// The registers values are held in, taken lowest first.
    const ALLOCATABLE: &'static [Self::Reg];
    /// The registers a call preserves, which hold values live across loops.
    const CALLEE_SAVED: &'static [Self::Reg];
    /// The registers kept for the framework's own moves, apart from any value: one carries a
    /// value between two places in memory, the other breaks cycles of moves.
    const CARRIER: Self::Reg;
    const TEMP: Self::Reg;
    /// The bytes of a home: a register's.
    const WORD: u64;
    /// The alignment of the stack at a call.
    const STACK_ALIGN: u64;

    fn index(r: Self::Reg) -> usize;

    fn copy(code: &mut Code<Self>, dst: Self::Reg, src: Self::Reg);
    /// Loads the home at `offset` from the frame pointer.
    fn load(code: &mut Code<Self>, dst: Self::Reg, offset: i32);
    fn store(code: &mut Code<Self>, offset: i32, src: Self::Reg);
    /// Writes a value recomputed where used into `dst`.
    fn recompute(code: &mut Code<Self>, dst: Self::Reg, what: Recompute);
    fn jump(code: &mut Code<Self>, to: vun_asm::Label);
}

/// The instruction compilers of an architecture: what the framework calls as it walks a
/// function.
pub trait Isel: Arch {
    /// The state of the instruction compilers for `f`.
    ///
    /// # Errors
    /// Returns what the backend cannot compile in `f`'s signature.
    fn state(f: &Function) -> Result<Self::State, Lack>;

    /// Whether a branch on the comparison `cond` jumps on the flags it sets.
    fn fuses(cond: Cond) -> bool;

    /// The prologue, and where the parameters passed in memory are (`Cx::recompute_as`,
    /// `Cx::home_at`).
    ///
    /// # Errors
    /// Returns what the backend cannot compile.
    fn prologue(cx: &mut Cx<'_, Self>) -> Result<(), Lack>;

    /// Receives the parameters where the convention passes them, once the callee-saved
    /// registers values keep are chosen.
    ///
    /// # Errors
    /// Returns what the backend cannot compile.
    fn entry(cx: &mut Cx<'_, Self>) -> Result<(), Lack>;

    /// # Errors
    /// Returns what the backend cannot compile.
    fn inst(cx: &mut Cx<'_, Self>, inst: Inst) -> Result<(), Lack>;

    /// # Errors
    /// Returns what the backend cannot compile.
    fn terminate(cx: &mut Cx<'_, Self>, block: Block) -> Result<(), Lack>;

    /// Writes what the function's end decides: the frame's size into the prologue.
    fn finish(cx: &mut Cx<'_, Self>, frame: u64);
}
