//! The stack of a block of a stack machine's instructions (spec 07, section 6): each instruction
//! pops the values its form says, of the types it says, and pushes its results; a block starts and
//! ends with an empty stack, except past an instruction that ends the code or leaves the block it
//! is in, after which any stack goes until that block ends. Blocks nest, each giving at most one value
//! (which a branch to it, but not to a loop, carries), and are branched to from inside only.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::ExprId;
use vun_inline::Inst;
use vun_isa::{Nest, Scope, Slot, Stack};
use vun_origin::OriginId;

use super::check::Checker;
use super::table::{Flow, Table};

/// A block of the code: how high the stack was when it opened, what kind it is, the value it
/// gives, and whether what follows is not reached, so that any values may be popped.
struct Frame {
    height: usize,
    nest: Nest,
    yields: Option<Slot>,
    unreachable: bool,
}

/// The stack of a block being checked, and the blocks it has open: the first frame is the block
/// of assembly itself.
struct Held {
    values: Vec<Slot>,
    frames: Vec<Frame>,
}

impl Held {
    fn frame(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("the block of assembly is open")
    }

    /// Pops a value of the type `want`, which an unreachable rest has wherever the stack is empty.
    fn take(&mut self, want: Slot) -> Result<(), &'static str> {
        let frame = self.frames.last().expect("the block of assembly is open");
        if self.values.len() == frame.height {
            return if frame.unreachable { Ok(()) } else { Err("pops a value off an empty stack") };
        }
        match (want, self.values.pop()) {
            (Slot::Class(a), Some(Slot::Class(b))) if a != b => {
                Err("pops a value of another type than it takes")
            }
            _ => Ok(()),
        }
    }

    /// Ends what follows as reached: only an end of the block starts it again.
    fn diverge(&mut self) {
        let height = self.frame().height;
        self.values.truncate(height);
        self.frame().unreachable = true;
    }

    /// The value a branch to the block `depth` blocks out carries, which a loop's start takes none
    /// of; or why there is no such block of this block of assembly.
    fn carried(&self, depth: u32) -> Result<Option<Slot>, &'static str> {
        let depth = depth as usize;
        if depth >= self.frames.len() - 1 {
            return Err("branches to a block this block of assembly did not open");
        }
        let frame = &self.frames[self.frames.len() - 1 - depth];
        Ok(if frame.nest == Nest::Loop { None } else { frame.yields })
    }

    /// Ends the arm of the innermost block: its stack holds what the block gives, or is not
    /// reached.
    fn arm(&mut self) -> Result<(), &'static str> {
        let (height, yields, unreachable) = {
            let frame = self.frame();
            (frame.height, frame.yields, frame.unreachable)
        };
        if let Some(slot) = yields {
            self.take(slot).map_err(|_| "ends a block without the value it gives")?;
        }
        if self.values.len() != height && !unreachable {
            return Err("ends a block with values on the stack");
        }
        self.values.truncate(height);
        Ok(())
    }

    /// What an instruction's `scope` does to the blocks open, or what it does wrong.
    fn scope(&mut self, scope: &Scope) -> Result<(), &'static str> {
        match scope {
            Scope::None => {}
            Scope::Open { nest, yields } => {
                let frame =
                    Frame { height: self.values.len(), nest: *nest, yields: *yields, unreachable: false };
                self.frames.push(frame);
            }
            Scope::Reopen => {
                if self.frame().nest != Nest::If || self.frames.len() == 1 {
                    return Err("continues an `if` that is not open");
                }
                self.arm()?;
                self.frame().nest = Nest::Plain;
                self.frame().unreachable = false;
            }
            Scope::Close => {
                if self.frames.len() == 1 {
                    return Err("ends a block this block of assembly did not open");
                }
                if self.frame().nest == Nest::If && self.frame().yields.is_some() {
                    return Err("ends an `if` giving a value with no `else`");
                }
                self.arm()?;
                let frame = self.frames.pop().expect("a block is open");
                self.values.extend(frame.yields);
            }
            Scope::Branch { depth, always } => {
                let carried = self.carried(*depth)?;
                if let Some(slot) = carried {
                    self.take(slot)?;
                }
                if *always {
                    self.diverge();
                } else {
                    self.values.extend(carried);
                }
            }
            Scope::Table(depths) => {
                let carried = depths.iter().map(|&d| self.carried(d)).collect::<Result<Vec<_>, _>>()?;
                let first = carried.first().copied().flatten();
                if carried.iter().any(|c| c.is_some() != first.is_some()) {
                    return Err("branches to blocks that give different values");
                }
                if let Some(slot) = first {
                    self.take(slot)?;
                }
                self.diverge();
            }
        }
        Ok(())
    }
}

impl Checker<'_, '_, '_> {
    /// Whether `insts`, with the effect `stacks` gives each, take and leave the stack as they
    /// must, and nest their blocks; where not, reports the first instruction that does not, or
    /// `at` for values or blocks left.
    pub(super) fn stack(
        &mut self,
        at: OriginId,
        insts: &[Inst<ExprId, ExprId, ExprId>],
        stacks: &[Option<Stack>],
    ) -> bool {
        let mut held = Held {
            values: Vec::new(),
            frames: vec![Frame { height: 0, nest: Nest::Plain, yields: None, unreachable: false }],
        };
        for (inst, stack) in insts.iter().zip(stacks) {
            let Some(stack) = stack else { return true };
            for &want in stack.pops.iter().rev() {
                if let Err(what) = held.take(want) {
                    return self.mismatch(inst.origin, what);
                }
            }
            held.values.extend(&stack.pushes);
            if let Err(what) = held.scope(&stack.scope) {
                return self.mismatch(inst.origin, what);
            }
            let form = self.table.form(inst.op, inst.form);
            if form.is_some_and(|f| Table::flow(self.arch, f) == Flow::Ends) {
                held.diverge();
            }
        }
        if held.frames.len() > 1 {
            return self.mismatch(at, "leaves a block open");
        }
        let unreachable = held.frame().unreachable;
        unreachable || held.values.is_empty() || self.mismatch(at, "ends with values on the stack")
    }

    /// Reports a stack that does not fit at `origin`; always `false`.
    fn mismatch(&mut self, origin: OriginId, what: &'static str) -> bool {
        let msg = Msg::new().text("this instruction ").text(what);
        self.diag.error(Diagnostic::error(errors::ASM_STACK, msg).at(origin, None));
        false
    }
}
