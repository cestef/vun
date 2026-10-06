//! The single-pass code generation framework (spec 13, section 8.3), after TPDE: what does not
//! depend on the architecture. LIR is read directly with its shared analyses; where each value
//! lives is planned first by the register allocator (`vun_regalloc`, from what the instruction
//! compilers say each instruction needs); blocks are then compiled in `Order`, instructions in
//! program order, each selected and encoded at once into a `vun_asm::Buffer` over the places
//! the plan gives.
//!
//! An architecture brings its registers and the framework's own moves ([`Arch`]), its
//! instruction compilers and what each instruction needs ([`Isel`]), and, for instructions
//! described as data, the operands and forms its assembler takes ([`snippet::Emits`]). The
//! framework brings the rest: the plan and its moves, homes, values recomputed where used
//! (constants and addresses), the callee-saved registers the plan uses, the frame, compare and
//! branch fusion, and the snippet encoder with what snippets need.

mod assembly;
mod cx;
mod frame;
mod pieces;
pub mod snippet;
mod values;

pub use assembly::Assembly;
pub use cx::{Cx, Fused, Layout, compile};
pub use frame::Frame;
pub use pieces::pieces;
pub use values::{Anchor, Place, Recompute};
pub use vun_regalloc::{Bank, Need, Needs, Out, PReg, Read, RegSet, Remade};

use vun_backend::Lack;
use vun_lir::{Address, Block, Cond, Function, Inst, Scalar};
use vun_regalloc::PReg as Planned;

/// A function's code as it is emitted.
pub type Code<A> = vun_asm::Buffer<<A as Arch>::Isa, Address>;
/// A processor of `A`'s instruction set, which its code is compiled for.
pub type Processor<A> = vun_asm::Processor<<<A as Arch>::Isa as vun_asm::Isa>::Feature>;

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
    /// The registers a call may overwrite that values are held in, the preferred first; values
    /// are also held in those the convention preserves, but the frame pointer.
    const ALLOCATABLE: &'static [Self::Reg];
    /// The frame pointer, which homes are reached from.
    const FRAME: Self::Reg;
    /// The registers kept for the framework's own moves, apart from any value: one carries a
    /// value between two places in memory, the other breaks cycles of moves.
    const CARRIER: Self::Reg;
    const TEMP: Self::Reg;
    /// The bytes of a home: a register's.
    const WORD: u64;
    /// The alignment of the stack at a call.
    const STACK_ALIGN: u64;

    fn index(r: Self::Reg) -> usize;

    /// `r` as the register allocator names it: by its index.
    ///
    /// # Panics
    /// Never: a bank has at most 64 registers.
    fn preg(r: Self::Reg) -> Planned {
        Planned(u8::try_from(Self::index(r)).expect("banks of at most 64 registers"))
    }
    /// The number DWARF gives `r` in call frame information.
    fn dwarf(r: Self::Reg) -> u16;

    fn copy(code: &mut Code<Self>, dst: Self::Reg, src: Self::Reg);
    /// Loads the home at `offset` from the frame pointer.
    fn load(code: &mut Code<Self>, dst: Self::Reg, offset: i32);
    fn store(code: &mut Code<Self>, offset: i32, src: Self::Reg);
    /// Writes a value recomputed where used into `dst`.
    fn recompute(code: &mut Code<Self>, dst: Self::Reg, what: Recompute);
    /// Whether instructions take `what`, of `bits` bits, in place of a register (an immediate,
    /// an address operand), so it is made where used; otherwise it is held in a register while
    /// one is free and made again rather than reloaded.
    fn in_place(what: Recompute, bits: u32) -> bool;
    fn jump(code: &mut Code<Self>, to: vun_asm::Label);

    /// The bank values of type `ty` are held in: the general registers, floats' bits included,
    /// unless the architecture keeps a float apart in the float bank, which has no registers:
    /// such a value lives in its home, as wide as the value, each instruction reading it bringing
    /// it into a register of its own (an `f64` where general registers hold 32 bits).
    #[must_use]
    fn bank(ty: Scalar) -> Bank {
        let _ = ty;
        Bank::General
    }
}

/// The instruction compilers of an architecture: what the framework calls as it walks a
/// function.
pub trait Isel: Arch {
    /// The variant of the architecture's C calling convention a target follows.
    type Convention: Copy + Send + Sync + 'static;

    /// The state of the instruction compilers for `f`, under `convention`.
    ///
    /// # Errors
    /// Returns what the backend cannot compile in `f`'s signature.
    fn state(f: &Function, convention: Self::Convention) -> Result<Self::State, Lack>;

    /// The general registers `convention` preserves across calls.
    fn preserved(convention: Self::Convention) -> &'static [Self::Reg];

    /// Whether a branch on the comparison `cond` jumps on the flags it sets.
    fn fuses(cond: Cond) -> bool;

    /// What the backend lacks to compile `inst`, asked of every instruction before anything is
    /// planned, as one it cannot compile says nothing of what it needs.
    ///
    /// # Errors
    /// Returns what it lacks.
    fn lacks(cx: &Cx<'_, Self>, inst: Inst) -> Result<(), Lack>;

    /// What `inst` needs of the places of its operands and result, which the plan provides.
    fn inst_needs(cx: &Cx<'_, Self>, inst: Inst, needs: &mut Needs);

    /// What the terminator of `block` needs of the values it reads itself.
    fn term_needs(cx: &Cx<'_, Self>, block: Block, needs: &mut Needs);

    /// Where the entry receives the function's parameters, those the prologue made recomputed
    /// as addresses aside.
    fn entry_needs(cx: &Cx<'_, Self>, needs: &mut Needs);

    /// The prologue, and where the parameters passed in memory are (`Cx::recompute_as`,
    /// `Cx::home_at`).
    ///
    /// # Errors
    /// Returns what the backend cannot compile.
    fn prologue(cx: &mut Cx<'_, Self>) -> Result<(), Lack>;

    /// Receives the parameters where the convention passes them, as the entry needs said.
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
