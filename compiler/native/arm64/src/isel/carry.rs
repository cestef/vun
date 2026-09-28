//! Values between where they are held, general registers and memory, and the carriers a
//! convention passes them in: a general register as it is, a floating-point register by its bits.

use vun_asm_arm64::{Emit, Op, Operand, Reg, Shift, ShiftKind, Size};
use vun_codegen::{Arch, pieces};
use vun_lir::Value;

use super::Sel;
use crate::abi::Carrier;
use crate::asm::{AsmExt, OpExt};
use crate::{Arm, FAR, Opnd};
use vun_native::width::{part, size_of};

impl Sel<'_, '_> {
    /// The bytes of `v` in a register.
    fn bytes(&self, v: Value) -> u64 {
        u64::from(size_of(self.f.ty(v), self.unit.pointer_bits).unwrap_or(Size::B64).bytes())
    }

    /// Puts `v` in `carrier`.
    pub(super) fn pass(&mut self, v: Value, carrier: Carrier) {
        match carrier {
            Carrier::Int(r) => self.reg_to(v, r),
            Carrier::Float(x) => {
                self.reg_to(v, Arm::CARRIER);
                let size = part(self.bytes(v));
                self.asm.emit(Op::Fmov, size, &[x.into(), Arm::CARRIER.into()]);
            }
        }
    }

    /// Takes the float `v` from the floating-point register `x`, holding it in a general
    /// register; one passed in a general register is where the plan takes it from.
    pub(super) fn receive_float(&mut self, v: Value, x: vun_asm_arm64::V) {
        let (d, size) = (self.def(v), part(self.bytes(v)));
        self.asm.emit(Op::Fmov, size, &[d.into(), x.into()]);
    }

    /// Loads the `bytes` bytes `disp` past `base` into `carrier`.
    pub(super) fn load_part(&mut self, carrier: Carrier, base: Reg, disp: i64, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.load_bytes(r, base, disp, bytes),
            Carrier::Float(x) => self.asm.access(Op::Ldr, part(bytes), x.into(), base, disp, FAR),
        }
    }

    /// Stores the low `bytes` bytes `carrier` holds `disp` past `base`.
    pub(super) fn store_part(&mut self, base: Reg, disp: i64, carrier: Carrier, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.store_bytes(base, disp, r, bytes),
            Carrier::Float(x) => self.asm.access(Op::Str, part(bytes), x.into(), base, disp, FAR),
        }
    }

    /// Loads `bytes` (1 to 8) bytes `disp` past `base` into `dst`, in pieces a load can take.
    fn load_bytes(&mut self, dst: Reg, base: Reg, disp: i64, bytes: u64) {
        if let Some(size) = Size::of_bytes(bytes) {
            self.asm.access(Op::Ldr, size, dst.into(), base, disp, FAR);
            return;
        }
        self.asm.emit(Op::Mov, Size::B64, &[dst.into(), Reg::Zr.into()]);
        for (size, at) in pieces(bytes, &Size::WIDEST, |s| u64::from(s.bytes())) {
            self.asm.access(Op::Ldr, size, Arm::TEMP.into(), base, disp + at, FAR);
            let shifted = Opnd::Shifted(Arm::TEMP, Shift { kind: ShiftKind::Lsl, amount: byte_shift(at) });
            let piece = if at == 0 { Arm::TEMP.into() } else { shifted };
            self.asm.emit(Op::Orr, Size::B64, &[dst.into(), dst.into(), piece]);
        }
    }

    /// Stores the low `bytes` (1 to 8) bytes of `src` `disp` past `base`.
    fn store_bytes(&mut self, base: Reg, disp: i64, src: Reg, bytes: u64) {
        if let Some(size) = Size::of_bytes(bytes) {
            self.asm.access(Op::Str, size, src.into(), base, disp, FAR);
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
            self.asm.access(Op::Str, size, piece.into(), base, disp + at, FAR);
        }
    }

    /// Copies `bytes` bytes from `from` to `to`, both registers holding addresses, through `via`,
    /// largest pieces first. A piece past what an access reaches moves both addresses up to it
    /// rather than building its offset, so no register but these is written, and `via` may be
    /// [`FAR`]; both addresses are as they were after.
    pub(super) fn copy_bytes(&mut self, to: Reg, from: Reg, bytes: u64, via: Reg) {
        let mut moved = 0;
        for (size, at) in pieces(bytes, &Size::WIDEST, |s| u64::from(s.bytes())) {
            let disp = at - moved;
            if !Op::Ldr.reaches(size, via.into(), from, disp) || !Op::Str.reaches(size, via.into(), to, disp)
            {
                self.asm.offset(from, from, at - moved);
                self.asm.offset(to, to, at - moved);
                moved = at;
            }
            self.asm.access(Op::Ldr, size, via.into(), from, at - moved, FAR);
            self.asm.access(Op::Str, size, via.into(), to, at - moved, FAR);
        }
        if moved != 0 {
            self.asm.offset(from, from, -moved);
            self.asm.offset(to, to, -moved);
        }
    }
}

/// The shift bringing a piece at byte `at` into place.
fn byte_shift(at: i64) -> u8 {
    u8::try_from(at * 8).expect("under 8 bytes")
}
