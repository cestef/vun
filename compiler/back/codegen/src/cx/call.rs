//! Calls, as the framework plans them. Where each argument goes follows from the convention's
//! placements (`vun_abi::Passing`): one passed in a general register is there when the call is
//! reached, a constant written there by the call itself; one passed elsewhere is read from
//! wherever the plan holds it. A call may write every register but those it preserves, so the
//! values live across it are held in those or in their homes.

use vun_abi::{Carrier, Loc, Passing, Ret};
use vun_lir::{Callee, ImportKind, Signature, Value};
use vun_regalloc::{Need, Needs, Out, Read};

use super::Cx;
use crate::Arch;

impl<'a, A: Arch> Cx<'a, A> {
    /// The signature `callee` is called with.
    ///
    /// # Panics
    /// Never for a unit the verifier accepts, whose calls call functions.
    #[must_use]
    pub fn signature(&self, callee: &'a Callee) -> &'a Signature {
        match callee {
            Callee::Func(f) => &self.unit.functions[*f].signature,
            Callee::Import(i) => match &self.unit.imports[*i].kind {
                ImportKind::Func(s) => s,
                ImportKind::Data => unreachable!("the verifier checks calls call functions"),
            },
            Callee::Indirect(_, s) => s,
        }
    }

    /// Writes the constant `bits` into `dst`.
    pub fn constant_to(&mut self, dst: A::Reg, bits: u128) {
        A::recompute(&mut self.asm, dst, crate::Recompute::Const(bits));
    }

    /// What a call passing `args` as `passing` needs, calling `target` when it calls through an
    /// address, and defining `result`.
    pub fn call_needs<F>(
        &self,
        passing: &Passing<Carrier<A::Reg, F>>,
        args: &[Value],
        target: Option<Value>,
        result: Option<Value>,
        needs: &mut Needs,
    ) {
        let kept = |v: Value| self.recomputed(v).is_none();
        let read = |needs: &mut Needs, v: Value, need: Need, late: bool| {
            if kept(v) {
                needs.uses.push(Read { value: v, need, late });
            }
        };
        for (&a, loc) in args.iter().zip(&passing.params) {
            match loc {
                Loc::Reg(Carrier::Int(r)) if kept(a) => read(needs, a, Need::Fixed(A::preg(*r)), false),
                // A constant is written where it goes by the call itself.
                Loc::Reg(Carrier::Int(r)) => needs.reserved = needs.reserved.with(A::preg(*r)),
                Loc::Reg(Carrier::Float(_)) | Loc::Stack { .. } | Loc::Copied { .. } => {
                    read(needs, a, Need::Any, false);
                }
                Loc::Split { parts, .. } => {
                    read(needs, a, Need::Any, false);
                    for part in parts {
                        if let Carrier::Int(r) = part.reg {
                            needs.reserved = needs.reserved.with(A::preg(r));
                        }
                    }
                }
                Loc::Indirect { at, .. } => {
                    read(needs, a, Need::Any, false);
                    if let Loc::Reg(Carrier::Int(r)) = **at {
                        needs.reserved = needs.reserved.with(A::preg(r));
                    }
                }
                // The address the result is written to once the call returns.
                Loc::Returned => read(needs, a, Need::Any, true),
            }
        }
        if let Some(target) = target {
            read(needs, target, Need::Any, false);
        }
        if let (Some(out), Ret::Reg(carrier)) = (result, &passing.ret) {
            let out_at = match carrier {
                Carrier::Int(r) => Out::Fixed(A::preg(*r)),
                Carrier::Float(_) => Out::Reg,
            };
            needs.defs.push((out, out_at));
        }
        needs.clobbers = self.machine.clobbered();
    }
}
