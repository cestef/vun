//! The registers and the stack a call takes, counted as Cranelift counts them.

use cranelift_codegen::ir::types;
use vun_abi::{Carrier, Loc, Part};
use vun_backend::Missing;
use vun_lir::{Record, Scalar};

use super::{Lane, Layout, Param, Parts, Pass, SLOT, Skip};

/// Registers handed out in order, and the stack arguments follow, as Cranelift places them.
pub(super) struct Cursor<'a, I, F> {
    ints: &'a [I],
    floats: &'a [F],
    used: (usize, usize),
    pub(super) stack: u64,
}

impl<'a, I: Copy + PartialEq, F: Copy + PartialEq> Cursor<'a, I, F> {
    pub(super) const fn new(ints: &'a [I], floats: &'a [F]) -> Self {
        Self { ints, floats, used: (0, 0), stack: 0 }
    }

    /// Takes the register `carrier` names, which is the next of its class or a later one, the
    /// registers between left to what [`Skip`] returned says.
    pub(super) fn reg(&mut self, carrier: Carrier<I, F>) -> Result<Vec<Skip>, Missing> {
        let (at, used, skip) = match carrier {
            Carrier::Int(r) => (self.ints.iter().position(|x| *x == r), &mut self.used.0, Skip::Int),
            Carrier::Float(r) => (self.floats.iter().position(|x| *x == r), &mut self.used.1, Skip::Float),
        };
        let at = at.filter(|at| at >= used).ok_or(Missing::Placement)?;
        let skips = vec![skip; at - *used];
        *used = at + 1;
        Ok(skips)
    }

    /// Takes every register of a class, as a value on the stack does that the convention keeps
    /// from them.
    pub(super) fn exhaust(&mut self, float: bool) -> Vec<Skip> {
        let (used, total, skip) = if float {
            (&mut self.used.1, self.floats.len(), Skip::Float)
        } else {
            (&mut self.used.0, self.ints.len(), Skip::Int)
        };
        let skips = vec![skip; total - *used];
        *used = total;
        skips
    }

    /// Words of the stack skipped to reach `offset`, which is past the arguments so far or at
    /// the end of them.
    pub(super) fn pad(&mut self, offset: u64) -> Result<Vec<Skip>, Missing> {
        if offset < self.stack || !(offset - self.stack).is_multiple_of(SLOT) {
            return Err(Missing::Placement);
        }
        let pad = usize::try_from((offset - self.stack) / SLOT).map_err(|_| Missing::Placement)?;
        Ok(vec![Skip::Stack; pad])
    }

    /// A scalar of `bytes` placed at `offset` of the stack arguments, which Cranelift does once
    /// every register of its class is taken.
    pub(super) fn spill(&mut self, float: bool, offset: u64, bytes: u64) -> Result<Vec<Skip>, Missing> {
        let skips = self.exhaust(float);
        let stride = bytes.max(SLOT);
        if offset != self.stack.next_multiple_of(stride) {
            return Err(Missing::Placement);
        }
        self.stack = offset + stride;
        Ok(skips)
    }

    /// A scalar placed by `loc`.
    pub(super) fn scalar(&mut self, scalar: Scalar, loc: &Loc<Carrier<I, F>>) -> Result<Vec<Skip>, Missing> {
        match loc {
            Loc::Reg(carrier) => self.reg(*carrier),
            &Loc::Stack { offset, bytes } => self.spill(scalar.is_float(), offset, bytes),
            _ => Err(Missing::Placement),
        }
    }

    /// The pieces of `record` travelling in `parts`, each in the register the cursor is at; the
    /// registers before the first, if skipped, are the skips.
    pub(super) fn lanes(
        &mut self,
        record: &Record,
        parts: &[Part<Carrier<I, F>>],
    ) -> Result<(Vec<Skip>, Parts), Missing> {
        let mut skips = Vec::new();
        let mut lanes = Vec::new();
        for (at, part) in parts.iter().enumerate() {
            let skipped = self.reg(part.reg)?;
            if at == 0 {
                skips = skipped;
            } else if !skipped.is_empty() {
                return Err(Missing::Placement);
            }
            let ty = match (part.reg, part.bytes) {
                (Carrier::Int(_), 1) => types::I8,
                (Carrier::Int(_), 2) => types::I16,
                (Carrier::Int(_), 3 | 4) => types::I32,
                (Carrier::Int(_), 5..=8) => types::I64,
                (Carrier::Float(_), 4) => types::F32,
                (Carrier::Float(_), 8) => types::F64,
                _ => return Err(Missing::Record),
            };
            lanes.push(Lane { offset: part.offset, bytes: part.bytes, ty });
        }
        Ok((skips, Parts { layout: record.into(), lanes }))
    }

    /// A record copied onto the stack at `offset`: as Cranelift's struct argument where the
    /// processor has them, else as words, which reach the stack once every general register is
    /// taken.
    pub(super) fn stacked(&mut self, record: &Record, offset: u64, structs: bool) -> Result<Param, Missing> {
        let layout = Layout::from(record);
        let mut skips = if structs { Vec::new() } else { self.exhaust(false) };
        skips.extend(self.pad(offset)?);
        self.stack = offset + layout.words();
        Ok(Param { skips, pass: if structs { Pass::Struct(layout) } else { Pass::Words(layout) } })
    }
}
