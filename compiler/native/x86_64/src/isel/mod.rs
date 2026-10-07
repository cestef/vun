//! The instruction compilers of x86-64: most operations are snippets; calls, memory copies,
//! loops of atomic operations, unsigned 64-bit float conversions, the frame and the
//! terminators are written here.

mod call;
mod carry;
mod entry;
mod float;
mod memory;
mod needs;
mod select;
mod term;

use vun_backend::{Lack, Missing};
use vun_codegen::snippet::{Args, Snippet};
use vun_codegen::{Cx, Isel, Needs};
use vun_lir::{Block, Cond, Function, Inst, Scalar, Value};

use super::abi::Passing;
use super::{X86, condition};
use select::{Look, Selection};
use vun_asm_x86_64::{Reg, Size};
use vun_native::inline::Inlined;

/// What the instruction compilers keep for one function.
pub struct State {
    convention: &'static vun_abi::Convention<vun_asm_x86_64::Reg, vun_asm_x86_64::Xmm>,
    /// Where the function's own signature passes each value.
    passing: Passing,
    /// Where the prologue's `sub rsp, frame` is, and its immediate, written once the size is known.
    frame_at: u64,
    frame_imm: u64,
    /// The home of the address a result is written through, which is given back.
    hidden: Option<i32>,
    /// Where a result returned in registers is gathered.
    gathered: Option<i32>,
}

impl State {
    /// The registers the function's convention passes floats in, which a call overwrites.
    pub(crate) fn float_args(&self) -> &'static [vun_asm_x86_64::Xmm] {
        self.convention.registers.float_args
    }
}

/// The instruction compilers over the function compiled.
struct Sel<'c, 'a>(&'c mut Cx<'a, X86>);

impl<'a> std::ops::Deref for Sel<'_, 'a> {
    type Target = Cx<'a, X86>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::ops::DerefMut for Sel<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl Isel for X86 {
    type Convention = &'static vun_abi::Convention<vun_asm_x86_64::Reg, vun_asm_x86_64::Xmm>;

    fn state(f: &Function, convention: Self::Convention) -> Result<State, Lack> {
        let passing = Passing::of(&f.signature, convention).map_err(|u| Missing::from(u).at(f.origin))?;
        Ok(State { convention, passing, frame_at: 0, frame_imm: 0, hidden: None, gathered: None })
    }

    /// Those System V preserves.
    fn preserved(convention: Self::Convention) -> &'static [Reg] {
        convention.registers.preserved
    }

    fn fuses(cond: Cond) -> bool {
        condition(cond).is_some()
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
        Sel(cx).frame(frame);
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
            Selection::Call { callee, args, kind } => self.call(callee, args, kind, out)?,
            Selection::ThreadAddr { data } => self.thread_addr(data)?,
            Selection::Fetch { op, addr, value, size } => self.fetch(op, addr, value, size),
            Selection::Copy { to, from, bytes } => self.copy(to, from, bytes),
            Selection::UnsignedToFloat { value, precision } => self.unsigned_to_float(value, precision, out),
            Selection::FloatToUnsigned { value, precision } => self.float_to_unsigned(value, precision, out),
            Selection::Asm(asm, check) => asm.emit(check, self.0)?,
        }
        Ok(())
    }

    /// The size of an integer, address or float value, or what the backend lacks for it.
    fn size(&self, v: Value) -> Result<Size, Lack> {
        self.look().size(v)
    }

    fn size_of(&self, scalar: Scalar) -> Result<Size, Lack> {
        self.look().size_of(scalar)
    }

    /// Emits `snippet` over `ins`, defining nothing.
    fn apply(&mut self, snippet: Snippet<X86>, ins: &[Value], size: Size) {
        self.snippet(snippet, &Args { ins, out: None, size, cond: None });
    }
}
