//! Values between where they are held, general registers and memory, and the carriers a
//! convention passes them in: a general register as it is, a floating-point register by its bits.

use vun_asm_arm64::{Emit, Op, Operand, Reg, Shift, ShiftKind, Size};
use vun_codegen::{Arch, pieces};
use vun_lir::Value;

use super::Sel;
use crate::arm64::abi::Carrier;
use crate::arm64::{Arm, FAR, Opnd, access, size_of};

/// The size of a float's bits of `bytes` bytes.
fn bits(bytes: u64) -> Size {
    if bytes > 4 { Size::B64 } else { Size::B32 }
}

impl Sel<'_, '_> {
    /// The bytes of `v` in a register.
    fn bytes(&self, v: Value) -> u64 {
        u64::from(size_of(self.f.ty(v)).unwrap_or(Size::B64).bytes())
    }

    /// Puts `v` in `carrier`.
    pub(super) fn pass(&mut self, v: Value, carrier: Carrier) {
        match carrier {
            Carrier::Int(r) => self.reg_to(v, r),
            Carrier::Fp(x) => {
                self.reg_to(v, Arm::CARRIER);
                let size = bits(self.bytes(v));
                self.asm.emit(Op::Fmov, size, &[x.into(), Arm::CARRIER.into()]);
            }
        }
    }

    /// Takes `v` from `carrier`, holding it in a general register.
    pub(super) fn receive(&mut self, v: Value, carrier: Carrier) {
        match carrier {
            Carrier::Int(r) => self.def_at(v, r),
            Carrier::Fp(x) => {
                let (d, size) = (self.def(v), bits(self.bytes(v)));
                self.asm.emit(Op::Fmov, size, &[d.into(), x.into()]);
            }
        }
    }

    /// Loads the `bytes` bytes `disp` past `base` into `carrier`.
    pub(super) fn load_part(&mut self, carrier: Carrier, base: Reg, disp: i64, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.load_bytes(r, base, disp, bytes),
            Carrier::Fp(x) => access(&mut self.asm, Op::Ldr, bits(bytes), x.into(), base, disp, FAR),
        }
    }

    /// Stores the low `bytes` bytes `carrier` holds `disp` past `base`.
    pub(super) fn store_part(&mut self, base: Reg, disp: i64, carrier: Carrier, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.store_bytes(base, disp, r, bytes),
            Carrier::Fp(x) => access(&mut self.asm, Op::Str, bits(bytes), x.into(), base, disp, FAR),
        }
    }

    /// Loads `bytes` (1 to 8) bytes `disp` past `base` into `dst`, in pieces a load can take.
    fn load_bytes(&mut self, dst: Reg, base: Reg, disp: i64, bytes: u64) {
        if let Some(size) = Size::of_bytes(bytes) {
            access(&mut self.asm, Op::Ldr, size, dst.into(), base, disp, FAR);
            return;
        }
        self.asm.emit(Op::Mov, Size::B64, &[dst.into(), Reg::Zr.into()]);
        for (size, at) in pieces(bytes, &Size::WIDEST, |s| u64::from(s.bytes())) {
            access(&mut self.asm, Op::Ldr, size, Arm::TEMP.into(), base, disp + at, FAR);
            let shifted = Opnd::Shifted(Arm::TEMP, Shift { kind: ShiftKind::Lsl, amount: byte_shift(at) });
            let piece = if at == 0 { Arm::TEMP.into() } else { shifted };
            self.asm.emit(Op::Orr, Size::B64, &[dst.into(), dst.into(), piece]);
        }
    }

    /// Stores the low `bytes` (1 to 8) bytes of `src` `disp` past `base`.
    fn store_bytes(&mut self, base: Reg, disp: i64, src: Reg, bytes: u64) {
        if let Some(size) = Size::of_bytes(bytes) {
            access(&mut self.asm, Op::Str, size, src.into(), base, disp, FAR);
            return;
        }
        for (size, at) in pieces(bytes, &Size::WIDEST, |s| u64::from(s.bytes())) {
            let piece = if at == 0 {
                src
            } else {
                let amount = Operand::Imm(i64::from(byte_shift(at)));
                self.asm.emit(Op::Lsr, Size::B64, &[Arm::TEMP.into(), src.into(), amount]);
                Arm::TEMP
            };
            access(&mut self.asm, Op::Str, size, piece.into(), base, disp + at, FAR);
        }
    }

    /// Copies `bytes` bytes from `from` to `to`, both registers holding addresses, through `via`,
    /// largest pieces first.
    pub(super) fn copy_bytes(&mut self, to: Reg, from: Reg, bytes: u64, via: Reg) {
        for (size, at) in pieces(bytes, &Size::WIDEST, |s| u64::from(s.bytes())) {
            access(&mut self.asm, Op::Ldr, size, via.into(), from, at, FAR);
            access(&mut self.asm, Op::Str, size, via.into(), to, at, FAR);
        }
    }
}

/// The shift bringing a piece at byte `at` into place.
fn byte_shift(at: i64) -> u8 {
    u8::try_from(at * 8).expect("under 8 bytes")
}
