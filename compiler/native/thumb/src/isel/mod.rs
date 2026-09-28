//! The instruction compilers of Thumb: most operations are snippets; calls, memory copies, loops
//! of atomic operations, the frame and the terminators are written here.

mod atomic;
mod call;
mod carry;
mod entry;
mod memory;
mod needs;
mod select;
mod term;

use vun_asm_thumb::{Reg, Size, V};
use vun_backend::{Lack, Missing};
use vun_codegen::snippet::Args;
use vun_codegen::{Cx, Isel, Needs};
use vun_lir::{Block, Cond, Function, Inst, Scalar};

use vun_abi::Convention;

use super::abi::Passing;
use super::{Flags, Thumb2};
use select::{Look, Selection};
use vun_native::inline::Inlined;

/// What the instruction compilers keep for one function.
pub struct State {
    /// The variant of the call standard the target follows.
    convention: &'static Convention<Reg, V>,
    /// Where the function's own signature passes each value.
    passing: Passing,
    /// Where the prologue's frame size is written once known.
    frame_at: u64,
    /// Where the prologue saved `FAR`.
    far_at: i32,
    /// Where a result returned in registers is gathered.
    gathered: Option<i32>,
}

impl State {
    /// The registers the function's convention passes floats in, which a call overwrites: none
    /// where it passes them in general registers.
    pub(crate) fn float_args(&self) -> &'static [V] {
        self.convention.registers.float_args
    }
}

/// The instruction compilers over the function compiled.
struct Sel<'c, 'a>(&'c mut Cx<'a, Thumb2>);

impl<'a> std::ops::Deref for Sel<'_, 'a> {
    type Target = Cx<'a, Thumb2>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::ops::DerefMut for Sel<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl Isel for Thumb2 {
    type Convention = &'static Convention<Reg, V>;

    fn state(f: &Function, convention: Self::Convention) -> Result<State, Lack> {
        let passing = Passing::of(&f.signature, convention).map_err(|u| Missing::from(u).at(f.origin))?;
        Ok(State { convention, passing, frame_at: 0, far_at: 0, gathered: None })
    }

    /// Those the call standard preserves, but `FAR`, which every prologue saves, and the frame
    /// pointer.
    fn preserved(_: Self::Convention) -> &'static [Reg] {
        &[Reg::R4, Reg::R5, Reg::R6, Reg::R8, Reg::R9, Reg::R10]
    }

    fn fuses(cond: Cond) -> bool {
        cond.holds().is_some()
    }

    fn lacks(cx: &Cx<'_, Self>, inst: Inst) -> Result<(), Lack> {
        Look(cx).selection(inst).map(drop)
    }

    fn inst_needs(cx: &Cx<'_, Self>, inst: Inst, needs: &mut Needs) {
        Look(cx).needs(inst, needs);
    }

    fn term_needs(cx: &Cx<'_, Self>, block: Block, needs: &mut Needs) {
        Look(cx).term_needs(block, needs);
    }

    fn entry_needs(cx: &Cx<'_, Self>, needs: &mut Needs) {
        Look(cx).entry_needs(needs);
    }

    fn prologue(cx: &mut Cx<'_, Self>) -> Result<(), Lack> {
        Sel(cx).prologue();
        Ok(())
    }

    fn entry(cx: &mut Cx<'_, Self>) -> Result<(), Lack> {
        Sel(cx).entry();
        Ok(())
    }

    fn inst(cx: &mut Cx<'_, Self>, inst: Inst) -> Result<(), Lack> {
        Sel(cx).inst(inst)
    }

    fn terminate(cx: &mut Cx<'_, Self>, block: Block) -> Result<(), Lack> {
        Sel(cx).terminate(block)
    }

    fn finish(cx: &mut Cx<'_, Self>, frame: u64) {
        Sel(cx).finish(frame);
    }
}

impl<'a> Sel<'_, 'a> {
    /// The function compiled, read without emitting.
    fn look(&self) -> Look<'_, 'a> {
        Look(self.0)
    }

    fn inst(&mut self, inst: Inst) -> Result<(), Lack> {
        let out = self.f.insts[inst].result;
        match self.look().selection(inst)? {
            Selection::Snippet { snippet, ins, size, cond } => {
                self.snippet(snippet, &Args { ins: &ins, out, size, cond });
            }
            Selection::Call { callee, args } => self.call(callee, args, out)?,
            Selection::Rmw { op, addr, value } => self.rmw(op, addr, value, out),
            Selection::CompareExchange { addr, expected, new } => {
                self.compare_exchange(addr, expected, new, out);
            }
            Selection::Copy { to, from, bytes } => self.copy(to, from, bytes),
            Selection::Asm(asm, check) => asm.emit(check, self.0)?,
        }
        Ok(())
    }

    fn size_of(&self, scalar: Scalar) -> Result<Size, Lack> {
        self.look().size_of(scalar)
    }
}
