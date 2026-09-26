//! The snippet encoder: a snippet's steps emitted in order, each over operands chosen as the
//! architecture's forms allow.

use vun_lir::Value;

use super::{Emits, Slot, Snippet};
use crate::cx::Cx;
use crate::values::Place;

/// The most operands a step has.
const OPERANDS: usize = 4;
/// The most scratch registers a snippet names.
const TEMPS: usize = 4;

/// What a snippet is applied to.
pub struct Args<'v, A: Emits> {
    /// The operation's operands, which `Slot::In` and `Slot::At` name by position.
    pub ins: &'v [Value],
    /// The value the operation defines.
    pub out: Option<Value>,
    pub size: A::Size,
    pub cond: Option<A::Cond>,
}

/// An operand as it was folded, which a register can replace.
enum Folded<A: Emits> {
    /// A constant or a value in its home, for the value itself.
    Value(Value),
    /// A known address, for memory at it.
    Address(Value),
    /// A constant, for the value in the register the instruction requires.
    Through(Value, A::Reg),
}

// By hand: a derive would ask the architecture itself to be `Copy`.
impl<A: Emits> Clone for Folded<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Emits> Copy for Folded<A> {}

/// A step's operands as chosen so far.
struct Operands<A: Emits> {
    chosen: [A::Operand; OPERANDS],
    folded: [Option<Folded<A>>; OPERANDS],
    len: usize,
}

impl<A: Emits> Operands<A> {
    fn new() -> Self {
        Self { chosen: [A::imm(0); OPERANDS], folded: [None; OPERANDS], len: 0 }
    }

    fn push(&mut self, operand: A::Operand, folded: Option<Folded<A>>) {
        self.chosen[self.len] = operand;
        self.folded[self.len] = folded;
        self.len += 1;
    }

    fn all(&self) -> &[A::Operand] {
        &self.chosen[..self.len]
    }
}

impl<A: Emits> Cx<'_, A> {
    /// Emits `snippet` over `args`, the result left in its register.
    ///
    /// # Panics
    /// Panics if a step takes no form even with every operand in a register, or names a slot
    /// its arguments lack: the snippet is wrong for its architecture.
    pub fn snippet(&mut self, snippet: Snippet<A>, args: &Args<'_, A>) {
        // Registers instructions require are freed first, so that nothing placed later blocks them.
        for step in snippet {
            for &r in A::implicit(step.op) {
                self.take(r);
            }
            for slot in step.slots {
                if let Slot::Fixed(r) | Slot::Through(r, _) = slot {
                    self.take(*r);
                }
            }
        }
        let mut temps: [Option<A::Reg>; TEMPS] = [None; TEMPS];
        let mut out: Option<A::Reg> = None;
        for (i, step) in snippet.iter().enumerate() {
            let size = A::size(step.width, args.size);
            if step.op == A::MOV
                && out.is_none()
                && let [Slot::Out, from] = step.slots
                && let Some(r) = self.adopt(*from, args, &temps, snippet, i)
            {
                out = Some(r);
                continue;
            }
            let mut ops = Operands::<A>::new();
            // The positions of the result, where this step writes it first.
            let mut first = 0u8;
            for slot in step.slots {
                match *slot {
                    // A result first written here is placed once the operands are, so that it
                    // may take the register of one read for the last time.
                    Slot::Out => {
                        if out.is_none() {
                            first |= 1 << ops.len;
                        }
                        ops.push(out.map_or(A::imm(0), A::reg), None);
                    }
                    Slot::In(k) => {
                        let (operand, folded) = self.input(args.ins[k as usize]);
                        ops.push(operand, folded);
                    }
                    Slot::At(k) => {
                        let v = args.ins[k as usize];
                        if let Some(mem) = self.place(v).and_then(A::mem) {
                            ops.push(mem, Some(Folded::Address(v)));
                        } else {
                            let base = self.reg(v);
                            ops.push(A::at(base), None);
                        }
                    }
                    Slot::Temp(k) => {
                        let r = match temps[k as usize] {
                            Some(r) => r,
                            None => *temps[k as usize].insert(self.scratch()),
                        };
                        ops.push(A::reg(r), None);
                    }
                    Slot::Fixed(r) => ops.push(A::reg(r), None),
                    Slot::Through(r, k) => {
                        let v = args.ins[k as usize];
                        if let Some(value) = self.imm(v) {
                            ops.push(A::imm(value), Some(Folded::Through(v, r)));
                        } else {
                            self.reg_to(v, r);
                            ops.push(A::reg(r), None);
                        }
                    }
                    Slot::Cond => {
                        ops.push(
                            A::cond(args.cond.expect("a snippet naming a condition is given one")),
                            None,
                        );
                    }
                    Slot::Is(operand) => ops.push(operand, None),
                }
            }
            if first != 0 {
                let r = *out.insert(self.result(snippet, i, args));
                for at in (0..ops.len).filter(|at| first & 1 << at != 0) {
                    ops.chosen[at] = A::reg(r);
                }
            }
            // Folded operands go to registers, first to last, until a form takes them.
            while !A::fits(step.op, size, ops.all()) {
                let Some(at) = ops.folded[..ops.len].iter().position(Option::is_some) else {
                    panic!("no form takes a snippet step's operands in registers")
                };
                ops.chosen[at] = match ops.folded[at].take().expect("found folded") {
                    Folded::Value(v) => A::reg(self.reg(v)),
                    Folded::Address(v) => A::at(self.reg(v)),
                    Folded::Through(v, r) => {
                        self.reg_to(v, r);
                        A::reg(r)
                    }
                };
            }
            A::emit(&mut self.asm, step.op, size, ops.all());
        }
    }

    /// `v` as an operand: its register if it is in one, else as a constant or in its home where
    /// a form takes it so, else brought into a register.
    fn input(&mut self, v: Value) -> (A::Operand, Option<Folded<A>>) {
        if self.held(v).is_none() {
            let folded = self
                .imm(v)
                .map(A::imm)
                .or_else(|| self.in_home(v).and_then(|home| A::mem(Place::frame(home))));
            if let Some(operand) = folded {
                return (operand, Some(Folded::Value(v)));
            }
        }
        (A::reg(self.reg(v)), None)
    }

    /// The result's register, first written by step `i`: the register of an operand the step
    /// reads for the last time, since an instruction reads its operands before it writes, else a
    /// new one.
    fn result(&mut self, snippet: Snippet<A>, i: usize, args: &Args<'_, A>) -> A::Reg {
        let out = args.out.expect("a snippet naming its result is given one");
        let dying = snippet[i].slots.iter().find_map(|slot| match *slot {
            Slot::In(k) | Slot::At(k) if !Self::read_after(snippet, i, k) => {
                let v = args.ins[k as usize];
                (self.held(v).is_some() && self.dies(v)).then_some(v)
            }
            _ => None,
        });
        match dying.and_then(|v| self.inherit(out, v)) {
            Some(r) => r,
            None => self.def(out),
        }
    }

    /// Whether a step after step `i` reads operand `k`.
    fn read_after(snippet: Snippet<A>, i: usize, k: u8) -> bool {
        snippet[i + 1..]
            .iter()
            .flat_map(|s| s.slots)
            .any(|s| matches!(*s, Slot::In(j) | Slot::At(j) if j == k))
    }

    /// The result's register where the move of step `i` into it needs no instruction: a dying
    /// operand's the snippet reads no more, a fixed register an instruction wrote, a scratch one.
    fn adopt(
        &mut self,
        from: Slot<A>,
        args: &Args<'_, A>,
        temps: &[Option<A::Reg>; TEMPS],
        snippet: Snippet<A>,
        i: usize,
    ) -> Option<A::Reg> {
        let out = args.out?;
        match from {
            Slot::In(k) => {
                if Self::read_after(snippet, i, k) {
                    None
                } else {
                    self.inherit(out, args.ins[k as usize])
                }
            }
            Slot::Fixed(r) => {
                self.def_at(out, r);
                Some(r)
            }
            Slot::Temp(k) => {
                let r = temps[k as usize]?;
                self.def_at(out, r);
                Some(r)
            }
            _ => None,
        }
    }
}
