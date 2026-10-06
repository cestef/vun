//! The control frames of a body: blocks opened and ended, and the branches to them, which carry
//! the values the block they reach gives.

use vun_isa::Nest;

use super::{Check, Frame};
use crate::module::ValType;
use crate::module::invalid::Why;

impl Check<'_> {
    pub(super) fn open(&mut self, nest: Nest, yields: Vec<ValType>) {
        self.frames.push(Frame { height: self.stack.len(), nest, yields, unreachable: false });
    }

    /// Ends the function's frame, which gives its results.
    pub(super) fn end_function(&mut self) -> Result<(), Why> {
        if self.frames.len() != 1 {
            return Err(Why::Unclosed);
        }
        self.pop_all(self.results)?;
        let frame = self.frames.pop().ok_or(Why::Unbalanced)?;
        if self.stack.len() == frame.height { Ok(()) } else { Err(Why::Leftover) }
    }

    /// Ends the innermost block's arm: its stack holds what the block gives and nothing else.
    pub(super) fn arm(&mut self) -> Result<(), Why> {
        let yields = self.frame().yields.clone();
        self.pop_all(&yields)?;
        if self.stack.len() == self.frame().height { Ok(()) } else { Err(Why::Leftover) }
    }

    /// The operand types a branch to `label` carries: the function's results for its own frame, a
    /// block's what it gives, a loop's none.
    pub(super) fn label(&self, label: u32) -> Result<&[ValType], Why> {
        let depth = label as usize;
        if depth >= self.frames.len() {
            return Err(Why::Label(label));
        }
        let at = self.frames.len() - 1 - depth;
        let frame = &self.frames[at];
        Ok(match (at, frame.nest) {
            (0, _) => self.results,
            (_, Nest::Loop) => &[],
            _ => &frame.yields,
        })
    }

    pub(super) fn branch(&mut self, label: u32) -> Result<(), Why> {
        let carried = self.label(label)?.to_vec();
        self.pop_all(&carried)?;
        Ok(())
    }

    pub(super) fn else_(&mut self) -> Result<(), Why> {
        if self.frame().nest != Nest::If {
            return Err(Why::Unbalanced);
        }
        self.arm()?;
        let height = self.frame().height;
        self.stack.truncate(height);
        let frame = self.frames.last_mut().expect("the function's frame is open");
        frame.nest = Nest::Plain;
        frame.unreachable = false;
        Ok(())
    }

    pub(super) fn end_block(&mut self) -> Result<(), Why> {
        if self.frames.len() == 1 || (self.frame().nest == Nest::If && !self.frame().yields.is_empty()) {
            return Err(Why::Unbalanced);
        }
        self.arm()?;
        let frame = self.frames.pop().ok_or(Why::Unbalanced)?;
        self.stack.truncate(frame.height);
        self.stack.extend(frame.yields);
        Ok(())
    }

    /// A branch to `default`, or, by an index, to one of a table's `cases`, which carry what it
    /// does.
    pub(super) fn jump(&mut self, table: Option<&[u32]>, default: u32) -> Result<(), Why> {
        if table.is_some() {
            self.pop(ValType::I32)?;
        }
        self.targets(table.unwrap_or_default(), default)
    }

    /// A branch to `default` or one of `cases`, which all carry as many values.
    pub(super) fn targets(&mut self, cases: &[u32], default: u32) -> Result<(), Why> {
        let arity = self.label(default)?.len();
        for &l in cases {
            if self.label(l)?.len() != arity {
                return Err(Why::Label(l));
            }
        }
        self.branch(default)?;
        self.unreachable();
        Ok(())
    }

    pub(super) fn br_if(&mut self, label: u32) -> Result<(), Why> {
        self.pop(ValType::I32)?;
        self.through(label)
    }

    /// A branch to `label` that may not be taken: what it carries stays.
    pub(super) fn through(&mut self, label: u32) -> Result<(), Why> {
        let carried = self.label(label)?.to_vec();
        self.pop_all(&carried)?;
        for t in carried {
            self.push(t);
        }
        Ok(())
    }
}
