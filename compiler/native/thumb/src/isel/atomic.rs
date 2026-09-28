//! Atomic read-modify-write operations and compare-and-exchange of a word, as loops of an
//! exclusive load and store between barriers, which make them sequentially consistent; the loop
//! runs again when another write came between the load and the store.

use vun_asm_thumb::{Barrier, Cc, Emit, Mem, Op, Operand, Reg, Size};
use vun_lir::{RmwOp, Value};

use super::Sel;

impl Sel<'_, '_> {
    /// `op` on the word at `addr` and `value`, giving what it held.
    pub(super) fn rmw(&mut self, op: RmwOp, addr: Value, value: Value, out: Option<Value>) {
        let (at, v) = (self.reg(addr), self.reg(value));
        let old = self.defined(out);
        let (new, status) = (self.scratch(), self.scratch());
        self.barrier();
        let again = self.asm.label();
        self.asm.bind(again);
        self.asm.emit(Op::Ldrex, Size::B32, &[old.into(), Mem::at(at, 0).into()]);
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
                self.asm.emit(combine, Size::B32, &[new.into(), old.into(), v.into()]);
                new
            }
        };
        self.store_exclusive(status, written, at, again);
        self.barrier();
    }

    /// Writes `new` at `addr` where it holds `expected`, giving what it held.
    pub(super) fn compare_exchange(&mut self, addr: Value, expected: Value, new: Value, out: Option<Value>) {
        let (at, e, n) = (self.reg(addr), self.reg(expected), self.reg(new));
        let old = self.defined(out);
        let status = self.scratch();
        let (again, done) = (self.asm.label(), self.asm.label());
        self.barrier();
        self.asm.bind(again);
        self.asm.emit(Op::Ldrex, Size::B32, &[old.into(), Mem::at(at, 0).into()]);
        self.asm.emit(Op::Cmp, Size::B32, &[old.into(), e.into()]);
        self.asm.emit(Op::Bcond, Size::B32, &[Cc::Ne.into(), done.into()]);
        self.store_exclusive(status, n, at, again);
        self.asm.bind(done);
        self.barrier();
    }

    /// Stores `value` at `at` if nothing wrote there since the exclusive load, going back to
    /// `again` otherwise.
    fn store_exclusive(&mut self, status: Reg, value: Reg, at: Reg, again: vun_asm::Label) {
        self.asm.emit(Op::Strex, Size::B32, &[status.into(), value.into(), Mem::at(at, 0).into()]);
        self.asm.emit(Op::Cmp, Size::B32, &[status.into(), Operand::Imm(0)]);
        self.asm.emit(Op::Bcond, Size::B32, &[Cc::Ne.into(), again.into()]);
    }

    /// Orders every access before it before any after.
    fn barrier(&mut self) {
        self.asm.emit(Op::Dmb, Size::B32, &[Operand::Barrier(Barrier::Sy)]);
    }

    /// The register of `out`, or a scratch one where the result is unused.
    fn defined(&mut self, out: Option<Value>) -> Reg {
        match out {
            Some(out) => self.def(out),
            None => self.scratch(),
        }
    }
}
