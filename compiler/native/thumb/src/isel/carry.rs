//! Values between where they are held, general registers and memory, and the carriers a
//! convention passes them in: a general register as it is, a floating-point register by its bits.

use vun_asm_thumb::{Emit, Op, Operand, Reg, Shift, ShiftKind, Size, V};
use vun_codegen::{Arch, pieces};
use vun_lir::Value;

use super::Sel;
use crate::abi::Carrier;
use crate::asm::AsmExt;
use crate::{FAR, Instr, Opnd, Thumb2};
use vun_native::width::part;

/// The sizes an access moves, widest first: a word at most.
const PIECES: [Size; 3] = [Size::B32, Size::B16, Size::B8];

impl Sel<'_, '_> {
    /// Puts `v` in `carrier`.
    pub(super) fn pass(&mut self, v: Value, carrier: Carrier) {
        match carrier {
            Carrier::Int(r) => self.reg_to(v, r),
            Carrier::Float(x) => {
                self.reg_to(v, Thumb2::CARRIER);
                self.asm.emit(Op::Vmov, Size::B32, &[x.into(), Thumb2::CARRIER.into()]);
            }
        }
    }

    /// Takes the float `v` from the floating-point register `x`, holding it in a general
    /// register; one passed in a general register is where the plan takes it from.
    pub(super) fn receive_float(&mut self, v: Value, x: V) {
        let d = self.def(v);
        self.asm.emit(Op::Vmov, Size::B32, &[d.into(), x.into()]);
    }

    /// Loads the `bytes` bytes `disp` past `base` into `carrier`.
    pub(super) fn load_part(&mut self, carrier: Carrier, base: Reg, disp: i64, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.load_bytes(r, base, disp, bytes),
            Carrier::Float(x) => self.asm.access(Op::Vldr, part(bytes), x.into(), base, disp, FAR),
        }
    }

    /// Stores the low `bytes` bytes `carrier` holds `disp` past `base`.
    pub(super) fn store_part(&mut self, base: Reg, disp: i64, carrier: Carrier, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.store_bytes(base, disp, r, bytes),
            Carrier::Float(x) => self.asm.access(Op::Vstr, part(bytes), x.into(), base, disp, FAR),
        }
    }

    /// Loads `bytes` (1 to 4) bytes `disp` past `base` into `dst`, in pieces a load can take.
    fn load_bytes(&mut self, dst: Reg, base: Reg, disp: i64, bytes: u64) {
        if let Some(size) = <Size as vun_asm::Width>::of_bytes(bytes) {
            self.access(Instr::Load, size, dst, base, disp);
            return;
        }
        self.asm.constant(dst, 0);
        for (size, at) in pieces(bytes, &PIECES, |s| u64::from(s.bytes())) {
            self.access(Instr::Load, size, Thumb2::TEMP, base, disp + at);
            let amount = u8::try_from(8 * at).expect("within a word");
            let shifted = Opnd::Shifted(Thumb2::TEMP, Shift { kind: ShiftKind::Lsl, amount });
            let piece = if at == 0 { Thumb2::TEMP.into() } else { shifted };
            self.asm.emit(Op::Orr, Size::B32, &[dst.into(), dst.into(), piece]);
        }
    }

    /// Stores the low `bytes` (1 to 4) bytes of `src` `disp` past `base`.
    fn store_bytes(&mut self, base: Reg, disp: i64, src: Reg, bytes: u64) {
        if let Some(size) = <Size as vun_asm::Width>::of_bytes(bytes) {
            self.access(Instr::Store, size, src, base, disp);
            return;
        }
        for (size, at) in pieces(bytes, &PIECES, |s| u64::from(s.bytes())) {
            let piece = if at == 0 {
                src
            } else {
                let amount = Operand::Imm(8 * at);
                self.asm.emit(Op::Lsr, Size::B32, &[Thumb2::TEMP.into(), src.into(), amount]);
                Thumb2::TEMP
            };
            self.access(Instr::Store, size, piece, base, disp + at);
        }
    }

    /// Copies `bytes` bytes from `from` to `to`, both registers holding addresses, through `via`,
    /// largest pieces first. A piece past what an access reaches moves both addresses up to it
    /// rather than building its offset, so no register but these is written, and `via` may be
    /// [`FAR`]; both addresses are as they were after.
    pub(super) fn copy_bytes(&mut self, to: Reg, from: Reg, bytes: u64, via: Reg) {
        let mut moved = 0;
        for (size, at) in pieces(bytes, &PIECES, |s| u64::from(s.bytes())) {
            let disp = at - moved;
            if !Instr::Load.reaches(size, via, from, disp) || !Instr::Store.reaches(size, via, to, disp) {
                self.asm.offset(from, from, at - moved);
                self.asm.offset(to, to, at - moved);
                moved = at;
            }
            self.access(Instr::Load, size, via, from, at - moved);
            self.access(Instr::Store, size, via, to, at - moved);
        }
        if moved != 0 {
            self.asm.offset(from, from, -moved);
            self.asm.offset(to, to, -moved);
        }
    }

    /// The access `instr` of `size` between `reg` and memory `disp` bytes past `base`.
    fn access(&mut self, instr: Instr, size: Size, reg: Reg, base: Reg, disp: i64) {
        let (op, size) = instr.at(size);
        self.asm.access(op, size, reg.into(), base, disp, FAR);
    }
}
