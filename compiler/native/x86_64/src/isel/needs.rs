//! What each instruction, terminator and the entry needs of the plan, from the selection the
//! instruction compilers emit: snippets say it by their slots, calls by where the convention
//! passes each value; the sequences written here say it themselves.

use vun_asm_x86_64::Reg;
use vun_codegen::snippet::Args;
use vun_codegen::{Arch, Need, Needs, Out};
use vun_lir::{Block, Callee, Conv, Inst, Terminator, Value};

use super::select::{Look, Selection};
use crate::abi::{Carrier, Loc, Passing, Ret};
use crate::snippets::CMP;
use crate::{X86, memory};
use vun_native::inline::Inlined;

impl Look<'_, '_> {
    /// What `inst` needs; nothing for one the backend cannot compile, which compiling reports.
    pub(super) fn needs(&self, inst: Inst, needs: &mut Needs) {
        let cx = self.0;
        let out = cx.f.insts[inst].result;
        let Ok(selection) = self.selection(inst) else { return };
        let reserve = |needs: &mut Needs, regs: &[Reg]| {
            needs.reserved = regs.iter().fold(needs.reserved, |set, &r| set.with(X86::preg(r)));
        };
        match selection {
            Selection::Snippet { snippet, ins, size, cond } => {
                cx.snippet_needs(snippet, &Args { ins: &ins, out, size, cond }, needs);
            }
            Selection::Call { callee, args } => {
                let signature = cx.signature(callee);
                let Ok(passing) = Passing::of(signature, cx.state.convention) else { return };
                let target = match callee {
                    Callee::Indirect(target, _) => Some(*target),
                    Callee::Func(_) | Callee::Import(_) => None,
                };
                cx.call_needs(&passing, args, target, out, needs);
                // A variadic C function reads the SSE registers used from `al`.
                if signature.conv == Conv::C {
                    reserve(needs, &[Reg::Rax]);
                }
            }
            Selection::Fetch { addr, value, .. } => {
                self.address(addr, needs);
                self.read(needs, value, Need::Any);
                reserve(needs, &[Reg::Rax, Reg::Rdx]);
                needs.defs.extend(out.map(|v| (v, Out::Fixed(X86::preg(Reg::Rax)))));
            }
            Selection::Copy { to, from, bytes } if bytes <= super::memory::UNROLLED => {
                self.address(to, needs);
                self.address(from, needs);
            }
            Selection::Copy { to, from, .. } => {
                reserve(needs, &[Reg::Rsi, Reg::Rdi, Reg::Rcx]);
                self.read(needs, from, Need::Any);
                self.read(needs, to, Need::Any);
            }
            Selection::Asm(asm, check) => asm.needs(check, cx, needs),
            Selection::UnsignedToFloat { value, .. } | Selection::FloatToUnsigned { value, .. } => {
                self.read(needs, value, Need::Any);
                needs.defs.extend(out.map(|v| (v, Out::Reg)));
            }
        }
    }

    /// Memory at the address `v`: folded where it is known, else read from a register.
    fn address(&self, v: Value, needs: &mut Needs) {
        if self.0.place(v).and_then(memory).is_none() {
            self.read(needs, v, Need::Reg);
        }
    }

    /// Reads `v` where `need` says, unless it is recomputed where used.
    fn read(&self, needs: &mut Needs, v: Value, need: Need) {
        if self.0.recomputed(v).is_none() {
            needs.read(v, need);
        }
    }

    /// What the terminator of `block` reads itself: a fused comparison's operands, a condition,
    /// a switched value, or a value returned where the convention returns it.
    pub(super) fn term_needs(&self, block: Block, needs: &mut Needs) {
        let cx = self.0;
        match cx.f.blocks[block].term {
            Terminator::Branch { cond, .. } => match cx.fused(block) {
                Some(fused) => {
                    let Ok(size) = self.size(fused.a) else { return };
                    let ins = [fused.a, fused.b];
                    cx.snippet_needs(CMP, &Args { ins: &ins, out: None, size, cond: None }, needs);
                }
                None => self.read(needs, cond, Need::Reg),
            },
            Terminator::Switch { value, .. } => self.read(needs, value, Need::Reg),
            Terminator::Return(Some(v)) => match cx.state.passing.ret {
                Ret::Reg(Carrier::Int(r)) => self.read(needs, v, Need::Fixed(X86::preg(r))),
                Ret::Reg(Carrier::Float(_)) => self.read(needs, v, Need::Any),
                Ret::None | Ret::Split(_) | Ret::Memory => {}
            },
            Terminator::Return(None) | Terminator::Jump(_) | Terminator::Unreachable => {}
        }
    }

    /// Where the entry receives each parameter: in its register, in a general register from an
    /// SSE one, or in its home on the stack; one passed in memory is an address recomputed.
    pub(super) fn entry_needs(&self, needs: &mut Needs) {
        let cx = self.0;
        let Some(entry) = cx.f.entry() else { return };
        for (&v, loc) in cx.f.blocks[entry].params.iter().zip(&cx.state.passing.params) {
            let out = match loc {
                Loc::Reg(Carrier::Int(r)) => Out::Fixed(X86::preg(*r)),
                Loc::Reg(Carrier::Float(_)) => Out::Reg,
                Loc::Stack { .. } => Out::Home,
                Loc::Copied { .. } | Loc::Split { .. } | Loc::Returned | Loc::Indirect { .. } => continue,
            };
            needs.defs.push((v, out));
        }
    }
}
