//! An instruction, and its text as GNU as reads it: the mnemonic, then its operands' names.

use std::fmt;

use crate::operand::Operand;
use crate::table::{Form, Op};

/// The most operands an instruction takes.
const MAX: usize = 2;

/// An instruction: its operation, the form it was checked against, and its operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inst {
    pub op: Op,
    pub form: &'static Form,
    operands: [Option<Operand>; MAX],
}

impl Inst {
    /// # Panics
    /// Panics past two operands, which no instruction takes.
    #[must_use]
    pub fn new(op: Op, form: &'static Form, operands: &[Operand]) -> Self {
        assert!(operands.len() <= MAX, "two operands at most");
        let mut all = [None; MAX];
        for (slot, &operand) in all.iter_mut().zip(operands) {
            *slot = Some(operand);
        }
        Self { op, form, operands: all }
    }

    pub fn operands(&self) -> impl Iterator<Item = Operand> {
        self.operands.into_iter().flatten()
    }
}

impl fmt::Display for Inst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.form.name.unwrap_or(self.op.mnemonic()))?;
        for (k, operand) in self.operands().enumerate() {
            f.write_str(if k == 0 { " " } else { ", " })?;
            f.write_str(operand.name())?;
        }
        Ok(())
    }
}
