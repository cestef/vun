//! Memory: loads and stores, whose addresses fold a slot's, a datum's and constant offsets;
//! atomic operations, ordered as x86-64 orders memory (every load acquires, every store
//! releases, a sequentially consistent store exchanges); copies.

use vun_backend::Lack;
use vun_lir::{Access, InstKind, Order, RmwOp, Value};

use super::super::encode::{Alu, Unary};
use super::super::{Base, Cc, Mem, Reg, Rm, Size};
use super::FnCompiler;

/// Copies up to this many bytes move through a register, larger ones with `rep movsb`.
const UNROLLED: u64 = 128;

impl FnCompiler<'_> {
    /// The memory the address `v` names, folded when recomputed.
    pub(super) fn mem(&mut self, v: Value) -> Mem {
        match self.address(v) {
            Some(mem) => mem,
            None => Mem::at(self.get(v), 0),
        }
    }

    /// `mem` with a register base moved to `Reg::TEMP`, for an instruction that writes fixed
    /// registers.
    fn pinned(&mut self, mem: Mem) -> Mem {
        match mem.base {
            Base::Reg(r) if r != Reg::Rbp => {
                self.asm.mov_rr(Reg::TEMP, r);
                Mem { base: Base::Reg(Reg::TEMP), ..mem }
            }
            _ => mem,
        }
    }

    pub(super) fn memory(&mut self, kind: &InstKind, result: Option<Value>) -> Result<(), Lack> {
        let out = || result.expect("the instruction defines a value");
        match kind {
            InstKind::Load { ty, addr, .. } => {
                let size = self.size_of(*ty)?;
                let mem = self.mem(*addr);
                let d = self.def(out());
                self.asm.load(size, d, mem);
            }
            InstKind::Store { value, addr, access } => {
                let size = self.size(*value)?;
                let mem = self.mem(*addr);
                let x = self.get(*value);
                if *access == Access::Atomic(Order::SeqCst) {
                    self.asm.mov_rr(Reg::SCRATCH, x);
                    self.asm.xchg(size, mem, Reg::SCRATCH);
                } else {
                    self.asm.store(size, mem, x);
                }
            }
            InstKind::Rmw { op, addr, value, .. } => self.rmw(*op, *addr, *value, out())?,
            InstKind::CompareExchange { addr, expected, new, .. } => {
                let size = self.size(*new)?;
                let mem = self.mem(*addr);
                let mem = self.pinned(mem);
                self.get_into(*new, Reg::SCRATCH);
                let e = self.get(*expected);
                self.take(Reg::Rax);
                self.asm.mov_rr(Reg::Rax, e);
                self.asm.lock_cmpxchg(size, mem, Reg::SCRATCH);
                self.bind(out(), Reg::Rax, false);
            }
            InstKind::Offset { base, offset } => {
                let b = self.get(*base);
                let d;
                if let Some(imm) = self.constant(*offset) {
                    d = self.def(out());
                    self.asm.lea(d, Mem::at(b, imm));
                } else {
                    let o = self.get(*offset);
                    d = self.def(out());
                    self.asm.mov_rr(d, b);
                    self.asm.alu(Alu::Add, Size::B64, d, o);
                }
            }
            InstKind::Copy { to, from, bytes } => self.copy(*to, *from, *bytes),
            InstKind::Const { .. } | InstKind::SlotAddr(_) | InstKind::Addr(_) => {
                unreachable!("recomputed where used")
            }
            InstKind::Binary { .. }
            | InstKind::Compare { .. }
            | InstKind::Cast { .. }
            | InstKind::Select { .. }
            | InstKind::Call { .. } => unreachable!("selected apart"),
        }
        Ok(())
    }

    /// `op` on memory, atomically, giving what it held.
    fn rmw(&mut self, op: RmwOp, addr: Value, value: Value, out: Value) -> Result<(), Lack> {
        let size = self.size(value)?;
        let mem = self.mem(addr);
        let mem = self.pinned(mem);
        self.get_into(value, Reg::SCRATCH);
        let alu = match op {
            RmwOp::Swap => {
                self.asm.xchg(size, mem, Reg::SCRATCH);
                None
            }
            RmwOp::Add | RmwOp::Sub => {
                if op == RmwOp::Sub {
                    self.asm.unary(Unary::Neg, size.wide(), Rm::Reg(Reg::SCRATCH));
                }
                self.asm.lock_xadd(size, mem, Reg::SCRATCH);
                None
            }
            RmwOp::And => Some(Alu::And),
            RmwOp::Or => Some(Alu::Or),
            RmwOp::Xor => Some(Alu::Xor),
        };
        let Some(alu) = alu else {
            let d = self.def(out);
            self.asm.mov_rr(d, Reg::SCRATCH);
            return Ok(());
        };
        // Retried until no other write came between the load and the exchange.
        self.take(Reg::Rax);
        self.take(Reg::Rdx);
        self.asm.load(size, Reg::Rax, mem);
        let again = self.asm.label();
        self.asm.bind(again);
        self.asm.mov_rr(Reg::Rdx, Reg::Rax);
        self.asm.alu(alu, size.wide(), Reg::Rdx, Reg::SCRATCH);
        self.asm.lock_cmpxchg(size, mem, Reg::Rdx);
        self.asm.jcc(Cc::Ne, again);
        self.bind(out, Reg::Rax, false);
        Ok(())
    }

    /// Copies `bytes` bytes, largest pieces first.
    fn copy(&mut self, to: Value, from: Value, bytes: u64) {
        let (to, from) = (self.mem(to), self.mem(from));
        if bytes > UNROLLED {
            self.asm.lea(Reg::TEMP, from);
            self.asm.lea(Reg::SCRATCH, to);
            for r in [Reg::Rsi, Reg::Rdi, Reg::Rcx] {
                self.take(r);
            }
            self.asm.mov_rr(Reg::Rsi, Reg::TEMP);
            self.asm.mov_rr(Reg::Rdi, Reg::SCRATCH);
            self.asm.mov_ri(Reg::Rcx, bytes);
            self.asm.rep_movsb();
            return;
        }
        let mut done = 0;
        for size in [Size::B64, Size::B32, Size::B16, Size::B8] {
            let n = u64::from(size.bytes());
            while bytes - done >= n {
                let at = i32::try_from(done).expect("unrolled copies are short");
                self.asm.load(size, Reg::SCRATCH, from.plus(at));
                self.asm.store(size, to.plus(at), Reg::SCRATCH);
                done += n;
            }
        }
    }
}
