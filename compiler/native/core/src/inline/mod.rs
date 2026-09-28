//! Inline assembly (spec 07, section 6) on every native architecture: each instruction encoded
//! by the form it was checked against, over the registers the plan gives its bindings. Every
//! register the block names or its forms use unnamed is reserved for the whole block, so no
//! binding is held there, but for an input read where it names; an output is written in a
//! register of its own, loaded first for `inout`, and stored through its address once the block
//! ends. Where the program checks its blocks (`AsmCheck`), each instruction writing memory first
//! has the address it writes checked, and a block that returns has its stack pointer compared
//! with the one it started with (spec 13, section 8.7). A value bound in a floating-point
//! register, which no value is held in between instructions, is moved there from the general
//! register the plan gives it, or loaded there for `inout`, in one of the convention's argument
//! registers the block leaves free. What differs between architectures, their tables and
//! operands, is [`Inline`].

mod block;

use vun_asm::{Blocks, Label, Place, Untaken, Via};
use vun_backend::{AsmOperand, Lack, Missing};
use vun_codegen::{Code, Cx, Isel, Needs};
use vun_inline::{ClassId, RegRef};
use vun_lir::{Address, AsmCheck, Output, Scalar, Value};

use vun_asm::Width;

use crate::width::size_of;

pub type Inst = vun_inline::Inst<Value, Output, Address>;
pub type Mem = vun_inline::Mem<Value, Output, Address>;

/// What a native architecture adds to its instruction set's facts about blocks (`Blocks`): how
/// it emits an instruction, and loads and stores an output.
pub trait Inline: Isel {
    /// The instruction set, as it reads blocks.
    type Table: Blocks<Reg = Self::Reg>;

    /// Whether memory a block names at a position of `kind` is one this code addresses.
    ///
    /// # Errors
    /// What it does not address.
    fn addressing(mem: &Mem, kind: Kind<Self>) -> Result<(), AsmOperand> {
        Self::Table::addressable(mem, kind)
    }

    /// Emits `inst` over `operands` by its form.
    ///
    /// # Errors
    /// The form does not encode them.
    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Encoded<Self>]) -> Result<(), Untaken>;

    /// Checks through `check.write` the `bytes` the next instruction writes at `at`, keeping every
    /// register and the flags as they were: the general registers a call may overwrite, and the
    /// low 64 bits of the floating-point registers `floats`, those of the block's. No value lives in
    /// another floating-point register between instructions, and no instruction of the table
    /// reads more of one.
    fn check_write(code: &mut Code<Self>, at: &Encoded<Self>, bytes: u32, floats: &[u8], check: &AsmCheck);

    /// Writes the stack pointer into `dst`.
    fn stack(code: &mut Code<Self>, dst: Self::Reg);

    /// Ends the program through `check.stack` unless the stack pointer is `before`.
    fn check_stack(code: &mut Code<Self>, before: Self::Reg, check: &AsmCheck);

    /// The floating-point registers a value bound in one may take: the arguments' of the
    /// function's convention, which a call overwrites.
    fn floats(cx: &Cx<'_, Self>) -> Vec<RegRef>;

    /// Moves the bits of a value in `src`, `size` wide, into `dst`, of any class.
    fn moved(code: &mut Code<Self>, dst: RegRef, src: Self::Reg, size: Size<Self>);

    /// Loads `r`, of any class, from `addr`, or stores it there, `size` wide.
    fn load_at(code: &mut Code<Self>, r: RegRef, addr: Self::Reg, size: Size<Self>);
    fn store_at(code: &mut Code<Self>, addr: Self::Reg, r: RegRef, size: Size<Self>);
}

/// The width of an operation or an operand of `A`.
pub type Size<A> = <<A as Inline>::Table as Blocks>::Size;
/// What an operand position of `A` takes.
pub type Kind<A> = <<A as Inline>::Table as Blocks>::Kind;
/// An operand as `A`'s assembler encodes it.
pub type Encoded<A> = <<A as Inline>::Table as Blocks>::Operand<Address>;

/// Where a block's operands are in the function compiled: its bindings in the registers the
/// plan gives them, in order, and its lines at `labels`.
pub struct Placed<'a> {
    regs: std::vec::IntoIter<RegRef>,
    labels: &'a [Label],
}

impl<T: Blocks> Place<T, Address> for Placed<'_> {
    fn bound(&mut self, _: ClassId) -> RegRef {
        self.regs.next().expect("a register for each binding")
    }

    fn line(&self, line: vun_inline::Line) -> T::Operand<Address> {
        T::label(self.labels[usize::from(line.0)])
    }

    /// An import through its entry in the global offset table.
    fn via(&self, target: Address) -> Via {
        match target {
            Address::Import(_) => Via::Got,
            Address::Func(_) | Address::Data(_) => Via::Direct,
        }
    }
}

/// A block of inline assembly, as an architecture encodes it.
pub trait Inlined {
    /// Whether every memory operand is one `A` addresses, and every binding has a register.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn check<A: Inline>(&self) -> Result<(), Lack>;

    /// Its inputs in registers, the addresses of its outputs, a register for each output, and
    /// the registers it names or its forms use unnamed kept for it, but where an input is read.
    /// A value recomputed where used takes a scratch register, or is written into the one it
    /// is fixed to.
    /// Where it is checked, a register keeps the stack pointer it starts with.
    fn needs<A: Inline>(&self, check: Option<&AsmCheck>, cx: &Cx<'_, A>, needs: &mut Needs);

    /// Emits it: each output given its register, loaded for `inout`; each instruction by its
    /// form; then each output stored, where the block goes on.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn emit<A: Inline>(&self, check: Option<&AsmCheck>, cx: &mut Cx<'_, A>) -> Result<(), Lack>;
}

/// The size an output of type `ty`, addresses of `pointer_bits`, is loaded and stored at, or
/// what the backend lacks for it.
fn sized<S: Width>(ty: Scalar, pointer_bits: u32, inst: &Inst) -> Result<S, Lack> {
    size_of(ty, pointer_bits).ok_or_else(|| {
        let missing = if ty == Scalar::I128 { Missing::Int128 } else { Missing::Half };
        missing.at(inst.origin)
    })
}
