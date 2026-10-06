//! The stack of a block of a stack machine's instructions (spec 07, section 6): each instruction
//! pops the values its form says, of the types it says, and pushes its results; a block starts and
//! ends with an empty stack, except past an instruction that ends the code or leaves the block it
//! is in, after which any stack goes until that block ends. Blocks nest, hold no values, and are
//! branched to from inside only.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::ExprId;
use vun_inline::Inst;
use vun_isa::{Scope, Slot, Stack};
use vun_origin::OriginId;

use super::check::Checker;
use super::table::{Flow, Table};

/// A block of the code: how high the stack was when it opened, whether an `else` may still come,
/// and whether what follows is not reached, so that any values may be popped.
struct Frame {
    height: usize,
    two_armed: bool,
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

    /// Pops a value, which an unreachable rest has of any type wherever the stack is empty;
    /// `None` where there is none to pop.
    fn pop(&mut self) -> Option<Slot> {
        let frame = self.frames.last().expect("the block of assembly is open");
        if self.values.len() == frame.height {
            return frame.unreachable.then_some(Slot::Any);
        }
        self.values.pop()
    }

    /// Ends what follows as reached: only an end of the block starts it again.
    fn diverge(&mut self) {
        let height = self.frame().height;
        self.values.truncate(height);
        self.frame().unreachable = true;
    }

    fn open(&mut self, two_armed: bool) {
        self.frames.push(Frame { height: self.values.len(), two_armed, unreachable: false });
    }

    /// What an instruction's `scope` does to the blocks open, or what it does wrong.
    fn scope(&mut self, scope: Scope) -> Result<(), &'static str> {
        match scope {
            Scope::None => {}
            Scope::Open { two_armed } => self.open(two_armed),
            Scope::Reopen => {
                if !self.frame().two_armed || self.frames.len() == 1 {
                    return Err("continues an `if` that is not open");
                }
                if self.values.len() != self.frame().height && !self.frame().unreachable {
                    return Err("ends the first arm of an `if` with values on the stack");
                }
                let height = self.frame().height;
                self.values.truncate(height);
                *self.frame() = Frame { height, two_armed: false, unreachable: false };
            }
            Scope::Close => {
                if self.frames.len() == 1 {
                    return Err("ends a block this block of assembly did not open");
                }
                let frame = self.frames.pop().expect("a block is open");
                if self.values.len() != frame.height && !frame.unreachable {
                    return Err("ends a block with values on the stack");
                }
                self.values.truncate(frame.height);
            }
            Scope::Branch { depth, always } => {
                if depth as usize >= self.frames.len() - 1 {
                    return Err("branches to a block this block of assembly did not open");
                }
                if always {
                    self.diverge();
                }
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
            frames: vec![Frame { height: 0, two_armed: false, unreachable: false }],
        };
        for (inst, stack) in insts.iter().zip(stacks) {
            let Some(stack) = stack else { return true };
            for &want in stack.pops.iter().rev() {
                let Some(top) = held.pop() else {
                    return self.mismatch(inst.origin, "pops a value off an empty stack");
                };
                if matches!((want, top), (Slot::Class(a), Slot::Class(b)) if a != b) {
                    return self.mismatch(inst.origin, "pops a value of another type than it takes");
                }
            }
            held.values.extend(&stack.pushes);
            if let Err(what) = held.scope(stack.scope) {
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
