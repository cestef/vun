//! What each instruction, terminator and the entry needs of the plan, from the selection the
//! instruction compilers emit: snippets say it by their slots, calls by where the convention
//! passes each value; the sequences written here say it themselves.

use vun_codegen::snippet::Args;
use vun_codegen::{Arch, Bank, Need, Needs, Out, Read};
use vun_lir::{Block, Callee, Inst, Terminator, Value};

use super::narrow;
use super::select::{Look, Selection};
use crate::Arm;
use crate::abi::{Carrier, Loc, Passing, Ret};
use vun_native::inline::Inlined;

impl Look<'_, '_> {
    /// What `inst` needs; nothing for one the backend cannot compile, which compiling reports.
    pub(super) fn needs(&self, inst: Inst, needs: &mut Needs) {
        let cx = self.0;
        let out = cx.f.insts[inst].result;
        let Ok(selection) = self.selection(inst) else { return };
        match selection {
            Selection::Snippet { snippet, ins, size, cond } => {
                cx.snippet_needs(snippet, &Args { ins: &ins, out, size, cond }, needs);
            }
            Selection::Call { callee, args } => {
                let Ok(passing) = Passing::of(cx.signature(callee), cx.state.convention) else { return };
                let target = match callee {
                    Callee::Indirect(target, _) => Some(*target),
                    Callee::Func(_) | Callee::Import(_) => None,
                };
                cx.call_needs(&passing, args, target, out, needs);
            }
            // A loop reading its operands again after the value held is loaded into the result.
            Selection::Rmw { addr, value, .. } => {
                self.read(needs, addr, true);
                self.read(needs, value, true);
                Self::result(needs, out, 2);
            }
            Selection::CompareExchange { addr, expected, new, .. } => {
                for v in [addr, expected, new] {
                    self.read(needs, v, true);
                }
                Self::result(needs, out, 1);
            }
            Selection::Copy { to, from, bytes } if bytes <= super::memory::UNROLLED => {
                self.read(needs, to, false);
                self.read(needs, from, false);
            }
            Selection::Asm(asm, check) => asm.needs(check, cx, needs),
            Selection::Copy { to, from, .. } => {
                needs.temps.extend([Bank::General; 3]);
                for v in [to, from] {
                    if cx.recomputed(v).is_none() {
                        needs.read(v, Need::Any);
                    }
                }
            }
        }
    }

    /// Reads `v` in a register, read again after the result is written when `late`, unless it
    /// is recomputed where used, which then takes a scratch register.
    fn read(&self, needs: &mut Needs, v: Value, late: bool) {
        if self.0.recomputed(v).is_none() {
            needs.uses.push(Read { value: v, need: Need::Reg, late });
        } else {
            needs.temps.push(Bank::General);
        }
    }

    /// The result `out` of a loop in a register, else a scratch register for it, and `scratch`
    /// more.
    fn result(needs: &mut Needs, out: Option<Value>, scratch: usize) {
        match out {
            Some(v) => needs.defs.push((v, Out::Reg)),
            None => needs.temps.push(Bank::General),
        }
        needs.temps.extend(std::iter::repeat_n(Bank::General, scratch));
    }

    /// What the terminator of `block` reads itself: a fused comparison's operands, a condition,
    /// a switched value, extended in a scratch register when narrow, or a value returned where
    /// the convention returns it.
    pub(super) fn term_needs(&self, block: Block, needs: &mut Needs) {
        let cx = self.0;
        let kept = |v: Value| cx.recomputed(v).is_none();
        match cx.f.blocks[block].term {
            Terminator::Branch { cond, .. } => match cx.fused(block) {
                Some(fused) => {
                    let Ok((snippet, size)) = self.flags(fused.cond, fused.a) else { return };
                    let ins = [fused.a, fused.b];
                    cx.snippet_needs(snippet, &Args { ins: &ins, out: None, size, cond: None }, needs);
                }
                None if kept(cond) => needs.read(cond, Need::Reg),
                None => {}
            },
            Terminator::Switch { value, .. } => {
                if kept(value) {
                    needs.read(value, Need::Reg);
                }
                if self.size(value).is_ok_and(narrow) {
                    needs.temps.push(Bank::General);
                }
            }
            Terminator::Return(Some(v)) if kept(v) => match cx.state.passing.ret {
                Ret::Reg(Carrier::Int(r)) => needs.read(v, Need::Fixed(Arm::preg(r))),
                Ret::Reg(Carrier::Float(_)) => needs.read(v, Need::Any),
                Ret::None | Ret::Split(_) | Ret::Memory => {}
            },
            Terminator::Return(_) | Terminator::Jump(_) | Terminator::Unreachable => {}
        }
    }

    /// Where the entry receives each parameter: in its register, in a general register from a
    /// floating-point one, or in its home on the stack, a record the caller copied as its
    /// address; one passed in memory is an address recomputed.
    pub(super) fn entry_needs(&self, needs: &mut Needs) {
        let cx = self.0;
        let Some(entry) = cx.f.entry() else { return };
        for (&v, loc) in cx.f.blocks[entry].params.iter().zip(&cx.state.passing.params) {
            let loc = match loc {
                Loc::Indirect { at, .. } => at.as_ref(),
                loc => loc,
            };
            let out = match loc {
                Loc::Reg(Carrier::Int(r)) => Out::Fixed(Arm::preg(*r)),
                Loc::Reg(Carrier::Float(_)) => Out::Reg,
                Loc::Stack { .. } => Out::Home,
                Loc::Copied { .. } | Loc::Split { .. } | Loc::Returned | Loc::Indirect { .. } => continue,
            };
            needs.defs.push((v, out));
        }
    }
}
