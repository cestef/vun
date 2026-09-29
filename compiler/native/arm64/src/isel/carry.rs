//! Values between where they are held, general registers and memory, and the carriers a
//! convention passes them in: a general register as it is, a floating-point register by its
//! bits; and the accesses the shared carriers are written with (`Carries`).

use vun_asm_arm64::{Emit, Op, Operand, Reg, Shift, ShiftKind, Size, V};
use vun_codegen::{Arch, Cx};
use vun_lir::{Ext, Value};
use vun_native::select::{Carries, Way};
use vun_native::width::{part, size_of};

use super::Sel;
use crate::abi::Carrier;
use crate::asm::{AsmExt, OpExt};
use crate::{Arm, FAR, Opnd};

impl Carries for Arm {
    const PIECES: &'static [Size] = &Size::WIDEST;
    const FAR: Reg = FAR;

    fn piece_bytes(size: Size) -> u64 {
        u64::from(size.bytes())
    }

    fn access(cx: &mut Cx<'_, Self>, way: Way, size: Size, reg: Reg, base: Reg, disp: i64) {
        cx.asm.access(Self::op(way), size, reg.into(), base, disp, FAR);
    }

    fn reaches(way: Way, size: Size, reg: Reg, base: Reg, disp: i64) -> bool {
        Self::op(way).reaches(size, reg.into(), base, disp)
    }

    fn float_access(cx: &mut Cx<'_, Self>, way: Way, x: V, bytes: u64, base: Reg, disp: i64) {
        cx.asm.access(Self::op(way), part(bytes), x.into(), base, disp, FAR);
    }

    fn clear(cx: &mut Cx<'_, Self>, r: Reg) {
        cx.asm.emit(Op::Mov, Size::B64, &[r.into(), Reg::Zr.into()]);
    }

    fn or_piece(cx: &mut Cx<'_, Self>, dst: Reg, piece: Reg, at: i64) {
        let amount = u8::try_from(at * 8).expect("under 8 bytes");
        let shifted =
            if at == 0 { piece.into() } else { Opnd::Shifted(piece, Shift { kind: ShiftKind::Lsl, amount }) };
        cx.asm.emit(Op::Orr, Size::B64, &[dst.into(), dst.into(), shifted]);
    }

    fn piece_of(cx: &mut Cx<'_, Self>, dst: Reg, src: Reg, at: i64) {
        cx.asm.emit(Op::Lsr, Size::B64, &[dst.into(), src.into(), Operand::Imm(at * 8)]);
    }

    fn offset(cx: &mut Cx<'_, Self>, dst: Reg, base: Reg, disp: i64) {
        cx.asm.offset(dst, base, disp);
    }
}

impl Arm {
    /// The access moving a register `way`.
    const fn op(way: Way) -> Op {
        match way {
            Way::Load => Op::Ldr,
            Way::Store => Op::Str,
        }
    }
}

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

    /// Extends an integer of `size` in `r` to 32 bits, as the C convention expects of callers
    /// and callees on some systems.
    pub(super) fn extend(&mut self, r: Reg, size: Size, ext: Ext) {
        if size < Size::B32 {
            let op = match ext {
                Ext::Zero => Op::Uxt,
                Ext::Sign => Op::Sxt,
            };
            self.asm.emit(op, size, &[r.into(), r.into()]);
        }
    }
}
