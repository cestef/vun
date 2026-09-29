//! Values between where they are held, general registers and memory, and the carriers a
//! convention passes them in: a general register as it is, a floating-point register by its
//! bits, a double from its home; and the accesses the shared carriers are written with
//! (`Carries`).

use vun_asm_thumb::{Emit, Op, Operand, Reg, Shift, ShiftKind, Size, V};
use vun_codegen::{Arch, Cx};
use vun_lir::{Ext, Value};
use vun_native::select::{Carries, Way};
use vun_native::width::part;

use super::Sel;
use crate::abi::Carrier;
use crate::asm::AsmExt;
use crate::{FAR, Instr, Opnd, Thumb2};

impl Carries for Thumb2 {
    /// A word at most.
    const PIECES: &'static [Size] = &[Size::B32, Size::B16, Size::B8];
    const FAR: Reg = FAR;

    fn piece_bytes(size: Size) -> u64 {
        u64::from(size.bytes())
    }

    fn access(cx: &mut Cx<'_, Self>, way: Way, size: Size, reg: Reg, base: Reg, disp: i64) {
        let (op, size) = Self::instr(way).at(size);
        cx.asm.access(op, size, reg.into(), base, disp, FAR);
    }

    fn reaches(way: Way, size: Size, reg: Reg, base: Reg, disp: i64) -> bool {
        Self::instr(way).reaches(size, reg, base, disp)
    }

    fn float_access(cx: &mut Cx<'_, Self>, way: Way, x: V, bytes: u64, base: Reg, disp: i64) {
        let op = match way {
            Way::Load => Op::Vldr,
            Way::Store => Op::Vstr,
        };
        cx.asm.access(op, part(bytes), x.into(), base, disp, FAR);
    }

    fn clear(cx: &mut Cx<'_, Self>, r: Reg) {
        cx.asm.constant(r, 0);
    }

    fn or_piece(cx: &mut Cx<'_, Self>, dst: Reg, piece: Reg, at: i64) {
        let amount = u8::try_from(8 * at).expect("within a word");
        let shifted =
            if at == 0 { piece.into() } else { Opnd::Shifted(piece, Shift { kind: ShiftKind::Lsl, amount }) };
        cx.asm.emit(Op::Orr, Size::B32, &[dst.into(), dst.into(), shifted]);
    }

    fn piece_of(cx: &mut Cx<'_, Self>, dst: Reg, src: Reg, at: i64) {
        cx.asm.emit(Op::Lsr, Size::B32, &[dst.into(), src.into(), Operand::Imm(8 * at)]);
    }

    fn offset(cx: &mut Cx<'_, Self>, dst: Reg, base: Reg, disp: i64) {
        cx.asm.offset(dst, base, disp);
    }
}

impl Thumb2 {
    /// The access moving a general register `way`.
    const fn instr(way: Way) -> Instr {
        match way {
            Way::Load => Instr::Load,
            Way::Store => Instr::Store,
        }
    }
}

impl Sel<'_, '_> {
    /// Puts `v` in `carrier`: a double from its home.
    pub(super) fn pass(&mut self, v: Value, carrier: Carrier) {
        match carrier {
            Carrier::Int(r) => self.reg_to(v, r),
            Carrier::Float(x) if self.homed(v) => self.load_double(x, v),
            Carrier::Float(x) => {
                self.reg_to(v, Thumb2::CARRIER);
                self.asm.emit(Op::Vmov, Size::B32, &[x.into(), Thumb2::CARRIER.into()]);
            }
        }
    }

    /// Takes the float `v` from the floating-point register `x`, holding it in a general
    /// register, or a double in its home; one passed in a general register is where the plan
    /// takes it from.
    pub(super) fn receive_float(&mut self, v: Value, x: V) {
        if self.homed(v) {
            self.store_double(v, x);
            return;
        }
        let d = self.def(v);
        self.asm.emit(Op::Vmov, Size::B32, &[d.into(), x.into()]);
    }

    /// Extends an integer of `size` in `r` to 32 bits, as the C convention expects of callers
    /// and callees.
    pub(super) fn extend(&mut self, r: Reg, size: Size, ext: Ext) {
        if size.narrow() {
            let extend = match ext {
                Ext::Zero => Instr::Zext,
                Ext::Sign => Instr::Sext,
            };
            let (op, size) = extend.at(size);
            self.asm.emit(op, size, &[r.into(), r.into()]);
        }
    }
}
