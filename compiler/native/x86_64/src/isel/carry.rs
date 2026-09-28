//! Values between where they are held, general registers and memory, and the carriers a
//! convention passes them in: a general register as it is, an SSE register by its bits.

use vun_asm_x86_64::{Emit, Mem, Op, Reg, Size, Xmm};
use vun_codegen::{Arch, pieces};
use vun_lir::Value;

use super::Sel;
use crate::X86;
use crate::abi::Carrier;
use vun_native::width::{part, size_of};

impl Sel<'_, '_> {
    /// The bytes of `v` in a register.
    fn bytes(&self, v: Value) -> u64 {
        u64::from(size_of(self.f.ty(v)).unwrap_or(Size::B64).bytes())
    }

    /// Puts `v` in `carrier`.
    pub(super) fn pass(&mut self, v: Value, carrier: Carrier) {
        match carrier {
            Carrier::Int(r) => self.reg_to(v, r),
            Carrier::Float(x) => {
                self.reg_to(v, X86::CARRIER);
                self.move_to_sse(part(self.bytes(v)), x, X86::CARRIER);
            }
        }
    }

    /// Takes the float `v` from the SSE register `x`, holding it in a general register; one
    /// passed in a general register is where the plan takes it from.
    pub(super) fn receive_float(&mut self, v: Value, x: Xmm) {
        let (d, size) = (self.def(v), part(self.bytes(v)));
        self.asm.emit(Op::Movd, size, &[d.into(), x.into()]);
    }

    fn move_to_sse(&mut self, size: Size, x: Xmm, r: Reg) {
        self.asm.emit(Op::Movd, size, &[x.into(), r.into()]);
    }

    /// Loads the `bytes` bytes at `at` into `carrier`: a float part, of 4 or 8 bytes, directly.
    pub(super) fn load_part(&mut self, carrier: Carrier, at: Mem<vun_lir::Address>, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.load_bytes(r, at, bytes),
            Carrier::Float(x) => self.asm.emit(Op::Movd, part(bytes), &[x.into(), at.into()]),
        }
    }

    /// Stores the low `bytes` bytes `carrier` holds at `at`, which a general register no longer
    /// holds after.
    pub(super) fn store_part(&mut self, at: Mem<vun_lir::Address>, carrier: Carrier, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.store_bytes(at, r, bytes),
            Carrier::Float(x) => self.asm.emit(Op::Movd, part(bytes), &[at.into(), x.into()]),
        }
    }

    /// Loads `bytes` (1 to 8) bytes at `at` into `dst`, in pieces a load can take, through
    /// [`X86::TEMP`], which `dst` and `at` must not be.
    fn load_bytes(&mut self, dst: Reg, at: Mem<vun_lir::Address>, bytes: u64) {
        if let Some(size) = Size::of_bytes(bytes) {
            self.asm.emit(Op::Movzx, size, &[dst.into(), at.into()]);
            return;
        }
        self.asm.emit(Op::Xor, Size::B32, &[dst.into(), dst.into()]);
        for (size, offset) in pieces(bytes, &Size::WIDEST, |s| u64::from(s.bytes())) {
            let piece = at.plus(offset).expect("records fit 2 GiB");
            self.asm.emit(Op::Movzx, size, &[X86::TEMP.into(), piece.into()]);
            self.asm.emit(Op::Shl, Size::B64, &[X86::TEMP.into(), (offset * 8).into()]);
            self.asm.emit(Op::Or, Size::B64, &[dst.into(), X86::TEMP.into()]);
        }
    }

    /// Stores the low `bytes` (1 to 8) bytes of `src` at `at`, lowest piece first, shifting each
    /// stored piece out of `src`: no other register is written, so `at` may be based on any.
    fn store_bytes(&mut self, at: Mem<vun_lir::Address>, src: Reg, bytes: u64) {
        let mut stored = 0;
        for (size, offset) in pieces(bytes, &Size::WIDEST, |s| u64::from(s.bytes())) {
            if offset > stored {
                self.asm.emit(Op::Shr, Size::B64, &[src.into(), ((offset - stored) * 8).into()]);
                stored = offset;
            }
            let piece = at.plus(offset).expect("records fit 2 GiB");
            self.asm.emit(Op::Mov, size, &[piece.into(), src.into()]);
        }
    }

    /// Copies `bytes` bytes from `from` to `to` through `via`, largest pieces first.
    pub(super) fn copy_bytes(
        &mut self,
        to: Mem<vun_lir::Address>,
        from: Mem<vun_lir::Address>,
        bytes: u64,
        via: Reg,
    ) {
        for (size, at) in pieces(bytes, &Size::WIDEST, |s| u64::from(s.bytes())) {
            let (src, dst) = (from.plus(at).expect("fits"), to.plus(at).expect("fits"));
            self.asm.emit(Op::Movzx, size, &[via.into(), src.into()]);
            self.asm.emit(Op::Mov, size, &[dst.into(), via.into()]);
        }
    }
}
