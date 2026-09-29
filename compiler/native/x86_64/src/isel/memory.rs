//! Memory: atomic operations x86-64 has no instruction for, as loops of compare-exchanges, and
//! copies; loads, stores and the other atomic operations are snippets. Addresses fold a slot's,
//! a datum's and constant offsets.

use vun_asm::{RelocKind, Via};
use vun_asm_x86_64::{Cc, Emit, Mem, Op, Reg, Size};
use vun_codegen::Arch;
use vun_lir::{Address, DataId, Value};

use super::Sel;
use crate::{X86, memory};

/// Copies up to this many bytes move through a register, larger ones with `rep movsb`.
pub(super) const UNROLLED: u64 = 128;

impl Sel<'_, '_> {
    /// The memory the address `v` names, folded where it is known.
    fn mem(&mut self, v: Value) -> Mem<Address> {
        match self.place(v).and_then(memory) {
            Some(mem) => mem,
            None => Mem::at(self.reg(v), 0),
        }
    }

    /// `op` on memory, atomically, giving what it held in `rax`: retried until no other write
    /// came between the load and the exchange.
    pub(super) fn fetch(&mut self, op: Op, addr: Value, value: Value, size: Size) {
        let mem = self.mem(addr);
        self.reg_to(value, X86::CARRIER);
        self.asm.emit(Op::Movzx, size, &[Reg::Rax.into(), mem.into()]);
        let again = self.asm.label();
        self.asm.bind(again);
        self.asm.emit(Op::Mov, Size::B64, &[Reg::Rdx.into(), Reg::Rax.into()]);
        self.asm.emit(op, size.max(Size::B32), &[Reg::Rdx.into(), X86::CARRIER.into()]);
        self.asm.emit(Op::Cmpxchg, size, &[mem.into(), Reg::Rdx.into()]);
        self.asm.emit(Op::J, Size::B64, &[Cc::Nz.into(), again.into()]);
    }

    /// The address of the calling thread's storage of `data`, in `rax`: its TLS descriptor's
    /// call gives its offset from the thread pointer, which the block's first word holds. A
    /// linker writes the offset itself where it knows it, in an executable, which is why the
    /// sequence is the one it recognizes.
    pub(super) fn thread_addr(&mut self, data: DataId) {
        let target = Address::Data(data);
        self.asm.emit(Op::Lea, Size::B64, &[Reg::Rax.into(), Mem::rip(target, Via::Descriptor).into()]);
        let call = self.asm.here();
        self.asm.relocate(call, target, RelocKind::TlsDescCall, 0);
        self.asm.emit(Op::Call, Size::B64, &[Mem::at(Reg::Rax, 0).into()]);
        self.asm.emit(Op::Add, Size::B64, &[Reg::Rax.into(), Mem::thread(0).into()]);
    }

    /// Copies `bytes` bytes, through a register or, past `UNROLLED`, with `rep movsb`.
    pub(super) fn copy(&mut self, to: Value, from: Value, bytes: u64) {
        if bytes <= UNROLLED {
            let (to, from) = (self.mem(to), self.mem(from));
            self.copy_bytes(to, from, bytes, X86::CARRIER);
            return;
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
