//! Atomic read-modify-write operations and compare-and-exchange, as loops of an exclusive load
//! and store (Armv8.0, which every arm64 processor runs): the load acquires, the store releases,
//! and the loop runs again when another write came between them.

use vun_asm_arm64::{Cc, Emit, Extend, Mem, Op, Operand, Reg, Size};
use vun_lir::{RmwOp, Value};

use super::Sel;

impl Sel<'_, '_> {
    /// `op` on the scalar of `size` at `addr` and `value`, giving what it held.
    pub(super) fn rmw(&mut self, op: RmwOp, addr: Value, value: Value, out: Option<Value>, size: Size) {
        let (at, v) = (self.reg(addr), self.reg(value));
        let old = self.defined(out);
        let (new, status) = (self.scratch(), self.scratch());
        let again = self.asm.label();
        self.asm.bind(again);
        self.asm.emit(Op::Ldaxr, size, &[old.into(), Mem::at(at, 0).into()]);
        let written = match op {
            RmwOp::Swap => v,
            RmwOp::Add | RmwOp::Sub | RmwOp::And | RmwOp::Or | RmwOp::Xor => {
                let combine = match op {
                    RmwOp::Add => Op::Add,
                    RmwOp::Sub => Op::Sub,
                    RmwOp::And => Op::And,
                    RmwOp::Or => Op::Orr,
                    _ => Op::Eor,
                };
                self.asm.emit(combine, size.max(Size::B32), &[new.into(), old.into(), v.into()]);
                new
            }
        };
        self.asm.emit(Op::Stlxr, size, &[status.into(), written.into(), Mem::at(at, 0).into()]);
        self.asm.emit(Op::Cbnz, Size::B32, &[status.into(), again.into()]);
    }

    /// Writes `new` at `addr` where it holds `expected`, giving what it held.
    pub(super) fn compare_exchange(
        &mut self,
        addr: Value,
        expected: Value,
        new: Value,
        out: Option<Value>,
        size: Size,
    ) {
        let (at, e, n) = (self.reg(addr), self.reg(expected), self.reg(new));
        let old = self.defined(out);
        let status = self.scratch();
        let (again, done) = (self.asm.label(), self.asm.label());
        self.asm.bind(again);
        self.asm.emit(Op::Ldaxr, size, &[old.into(), Mem::at(at, 0).into()]);
        // The exclusive load zero-extends; the expected value is read at its width.
        let expected: Operand<vun_lir::Address> = match size {
            Size::B8 => Operand::Extended(e, Extend::Uxtb, 0),
            Size::B16 => Operand::Extended(e, Extend::Uxth, 0),
            _ => e.into(),
        };
        self.asm.emit(Op::Cmp, size.max(Size::B32), &[old.into(), expected]);
        self.asm.emit(Op::Bcond, Size::B64, &[Cc::Ne.into(), done.into()]);
        self.asm.emit(Op::Stlxr, size, &[status.into(), n.into(), Mem::at(at, 0).into()]);
        self.asm.emit(Op::Cbnz, Size::B32, &[status.into(), again.into()]);
        self.asm.bind(done);
    }

    /// The register of `out`, or a scratch one where the result is unused.
    fn defined(&mut self, out: Option<Value>) -> Reg {
        match out {
            Some(out) => self.def(out),
            None => self.scratch(),
        }
    }
}
