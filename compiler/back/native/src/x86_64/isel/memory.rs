//! Memory: loads and stores, whose addresses fold a slot's, a datum's and constant offsets;
//! atomic operations, ordered as x86-64 orders memory (every load acquires, every store
//! releases, a sequentially consistent store exchanges); copies.

use vun_asm_x86_64::{Cc, Emit, Mem, Op, Reg, Size};
use vun_backend::Lack;
use vun_codegen::Arch;
use vun_lir::{Access, Address, InstKind, Order, RmwOp, Value};

use super::Sel;
use crate::x86_64::snippets::{
    ADD, COMPARE_EXCHANGE, FETCH_ADD, FETCH_SUB, LOAD, STORE, STORE_SEQ_CST, SWAP,
};
use crate::x86_64::{X86, memory};

/// Copies up to this many bytes move through a register, larger ones with `rep movsb`.
const UNROLLED: u64 = 128;

impl Sel<'_, '_> {
    pub(super) fn memory(&mut self, kind: &InstKind, out: Option<Value>) -> Result<(), Lack> {
        match *kind {
            InstKind::Load { ty, addr, .. } => {
                let size = self.size_of(ty)?;
                self.apply(LOAD, &[addr], out, size, None);
            }
            InstKind::Store { value, addr, access } => {
                let size = self.size(value)?;
                let snippet = if access == Access::Atomic(Order::SeqCst) { STORE_SEQ_CST } else { STORE };
                self.apply(snippet, &[value, addr], None, size, None);
            }
            InstKind::Rmw { op, addr, value, .. } => {
                let size = self.size(value)?;
                match op {
                    RmwOp::Swap => self.apply(SWAP, &[addr, value], out, size, None),
                    RmwOp::Add => self.apply(FETCH_ADD, &[addr, value], out, size, None),
                    RmwOp::Sub => self.apply(FETCH_SUB, &[addr, value], out, size, None),
                    RmwOp::And => self.fetch(Op::And, addr, value, out, size),
                    RmwOp::Or => self.fetch(Op::Or, addr, value, out, size),
                    RmwOp::Xor => self.fetch(Op::Xor, addr, value, out, size),
                }
            }
            InstKind::CompareExchange { addr, expected, new, .. } => {
                let size = self.size(new)?;
                self.apply(COMPARE_EXCHANGE, &[addr, expected, new], out, size, None);
            }
            InstKind::Offset { base, offset } => self.apply(ADD, &[base, offset], out, Size::B64, None),
            InstKind::Copy { to, from, bytes } => self.copy(to, from, bytes),
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

    /// The memory the address `v` names, folded where it is known.
    fn mem(&mut self, v: Value) -> Mem<Address> {
        match self.place(v).and_then(memory) {
            Some(mem) => mem,
            None => Mem::at(self.reg(v), 0),
        }
    }

    /// `op` on memory, atomically, giving what it held: retried until no other write came
    /// between the load and the exchange.
    fn fetch(&mut self, op: Op, addr: Value, value: Value, out: Option<Value>, size: Size) {
        self.take(Reg::Rax);
        self.take(Reg::Rdx);
        let mem = self.mem(addr);
        self.reg_to(value, X86::CARRIER);
        self.asm.emit(Op::Movzx, size, &[Reg::Rax.into(), mem.into()]);
        let again = self.asm.label();
        self.asm.bind(again);
        self.asm.emit(Op::Mov, Size::B64, &[Reg::Rdx.into(), Reg::Rax.into()]);
        self.asm.emit(op, size.max(Size::B32), &[Reg::Rdx.into(), X86::CARRIER.into()]);
        self.asm.emit(Op::Cmpxchg, size, &[mem.into(), Reg::Rdx.into()]);
        self.asm.emit(Op::J, Size::B64, &[Cc::Ne.into(), again.into()]);
        if let Some(out) = out {
            self.def_at(out, Reg::Rax);
        }
    }

    /// Copies `bytes` bytes, through a register or, past `UNROLLED`, with `rep movsb`.
    fn copy(&mut self, to: Value, from: Value, bytes: u64) {
        if bytes <= UNROLLED {
            let (to, from) = (self.mem(to), self.mem(from));
            self.copy_bytes(to, from, bytes, X86::CARRIER);
            return;
        }
        for r in [Reg::Rsi, Reg::Rdi, Reg::Rcx] {
            self.take(r);
        }
        self.reg_to(from, Reg::Rsi);
        self.reg_to(to, Reg::Rdi);
        self.asm.emit(
            Op::Mov,
            Size::B64,
            &[Reg::Rcx.into(), i64::try_from(bytes).expect("copies fit memory").into()],
        );
        self.asm.emit(Op::Movsb, Size::B8, &[]);
    }
}
