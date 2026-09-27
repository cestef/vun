//! What a snippet needs, read from its slots: each operand in a register, or anywhere where every
//! step reading it takes it as memory, read again after the result is written when a later step
//! reads it; the result in the register of the operand its
//! first step moves into it where that step reads it last, in a fixed register it moves it from,
//! else in any; a scratch register per `Temp` and per value recomputed where used that may not
//! fold; the registers its instructions name or use without naming, taken while it runs.

use vun_regalloc::{Bank, Need, Needs, Out, Read};

use super::{Args, Emits, Slot, Snippet, Step};
use crate::cx::Cx;
use crate::values::Place;

impl<A: Emits> Cx<'_, A> {
    /// Adds what `snippet` over `args` needs to `needs`.
    pub fn snippet_needs(&self, snippet: Snippet<A>, args: &Args<'_, A>, needs: &mut Needs) {
        let written = snippet.iter().position(|step| step.slots.iter().any(|s| matches!(s, Slot::Out)));
        // The step an operand is read at last, and whether a scratch register brings it.
        let last = |k: u8| {
            snippet.iter().rposition(|s| {
                s.slots
                    .iter()
                    .any(|&slot| matches!(slot, Slot::In(j) | Slot::At(j) | Slot::Through(_, j) if j == k))
            })
        };
        // The operand a move gives the result the register of, which it is then read in.
        let reused = written.and_then(|w| match snippet[w].slots {
            [Slot::Out, Slot::In(k)] if snippet[w].op == A::MOV && last(*k) == Some(w) => Some(*k),
            _ => None,
        });
        let mut read = Vec::new();
        let mut in_memory = Vec::new();
        for step in snippet {
            for &slot in step.slots {
                match slot {
                    Slot::In(k) | Slot::At(k) | Slot::Through(_, k) if !read.contains(&k) => {
                        read.push(k);
                        let v = args.ins[k as usize];
                        let late = written.is_some_and(|w| last(k).is_some_and(|l| l > w));
                        match (slot, self.recomputed(v).is_some()) {
                            (Slot::Through(r, _), false) => {
                                needs.uses.push(Read { value: v, need: Need::Fixed(A::preg(r)), late });
                            }
                            (Slot::Through(r, _), true) => needs.reserved = needs.reserved.with(A::preg(r)),
                            // Read after the result is written, it stays in the register the
                            // plan reads it in.
                            (Slot::In(_), false)
                                if !late
                                    && Some(k) != reused
                                    && self.read_in_memory(snippet, args, k, &in_memory) =>
                            {
                                in_memory.push(k);
                                needs.uses.push(Read { value: v, need: Need::Any, late });
                            }
                            (_, false) => needs.uses.push(Read { value: v, need: Need::Reg, late }),
                            (_, true) => needs.temps.push(Bank::General),
                        }
                    }
                    Slot::Fixed(r) => needs.reserved = needs.reserved.with(A::preg(r)),
                    _ => {}
                }
            }
            for &r in A::implicit(step.op) {
                needs.reserved = needs.reserved.with(A::preg(r));
            }
        }
        let temps = snippet.iter().flat_map(|s| s.slots).filter_map(|s| match s {
            Slot::Temp(k) => Some(*k),
            _ => None,
        });
        let mut named: Vec<u8> = temps.collect();
        named.sort_unstable();
        named.dedup();
        needs.temps.extend(named.iter().map(|_| Bank::General));
        if let (Some(out), Some(w)) = (args.out, written) {
            let at = match snippet[w].slots {
                [Slot::Out, Slot::In(k)] if snippet[w].op == A::MOV && last(*k) == Some(w) => {
                    let v = args.ins[*k as usize];
                    needs.uses.iter().position(|r| r.value == v && !r.late).map_or(Out::Reg, Out::Reuse)
                }
                [Slot::Out, Slot::Fixed(r)] if snippet[w].op == A::MOV => Out::Fixed(A::preg(*r)),
                _ => Out::Reg,
            };
            needs.defs.push((out, at));
        }
    }

    /// Whether every step reading operand `k` of `snippet` takes it as memory, beside what the
    /// encoder makes of the others (a constant as an immediate, else a register), and none of
    /// them reads in memory an operand of `in_memory` too: one it may leave in its home.
    fn read_in_memory(&self, snippet: Snippet<A>, args: &Args<'_, A>, k: u8, in_memory: &[u8]) -> bool {
        // A value the operation reads twice is read in a register for the other.
        let v = args.ins[k as usize];
        let Some(home) =
            A::mem(Place::frame(0)).filter(|_| args.ins.iter().filter(|&&w| w == v).count() == 1)
        else {
            return false;
        };
        let reading = |step: &Step<A>| {
            step.slots.iter().any(|s| matches!(*s, Slot::In(j) | Slot::At(j) | Slot::Through(_, j) if j == k))
        };
        let mut steps = snippet.iter().filter(|step| reading(step)).peekable();
        steps.peek().is_some()
            && steps.all(|step| {
                let operands: Option<Vec<A::Operand>> = step
                    .slots
                    .iter()
                    .map(|&slot| match slot {
                        Slot::In(j) if j == k => Some(home),
                        Slot::In(j) if in_memory.contains(&j) => None,
                        Slot::In(j) => Some(self.imm(args.ins[j as usize]).map_or(A::reg(A::TEMP), A::imm)),
                        Slot::At(j) | Slot::Through(_, j) if j == k => None,
                        Slot::At(_) => Some(A::at(A::TEMP)),
                        Slot::Cond => args.cond.map(A::cond),
                        Slot::Is(operand) => Some(operand),
                        Slot::Out | Slot::Temp(_) | Slot::Fixed(_) | Slot::Through(..) => {
                            Some(A::reg(A::TEMP))
                        }
                    })
                    .collect();
                operands.is_some_and(|ops| A::fits(step.op, A::size(step.width, args.size), &ops))
            })
    }
}
