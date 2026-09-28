//! What Thumb code needs beyond one instruction: constants, offsets, addresses and accesses in as
//! few instructions as the forms take, and the frame size a prologue builds.

use vun_asm::imm::truncate;
use vun_asm_thumb::{Asm, Emit, Mem, Op, Reg, Size};
use vun_codegen::Arch;
use vun_lir::Address;

use super::{FAR, Opnd, Thumb2};

/// The bits of a half of a word, which `movw` and `movt` each move.
const HALF: u32 = 16;

/// Sequences of instructions over a Thumb code buffer.
pub(crate) trait AsmExt {
    /// Writes the low 32 bits of `value` into `dst`: in one move or one move of their complement
    /// where a form takes it, else `movw` of the low half and `movt` of the high one.
    fn constant(&mut self, dst: Reg, value: u64);
    /// Writes `base + disp` into `dst`, the constant built in `dst` or, when that is the base or
    /// the stack pointer, in `FAR`.
    fn offset(&mut self, dst: Reg, base: Reg, disp: i64);
    /// Writes the address of `target` into `dst`, half by half.
    fn address(&mut self, dst: Reg, target: Address);
    /// `op` of `size` between `reg` and memory `disp` bytes past `base`: at an offset where a form
    /// takes it, else at an index built in `scratch`, or at the address built there.
    fn access(&mut self, op: Op, size: Size, reg: Opnd, base: Reg, disp: i64, scratch: Reg);
    /// Two moves building the frame's `size`, negated, in `dst`, which the function's end
    /// rewrites once the size is known.
    fn frame_size(&mut self, dst: Reg, size: u64);
}

impl AsmExt for Asm<Address> {
    fn constant(&mut self, dst: Reg, value: u64) {
        let bits = u32::try_from(truncate(value.cast_signed(), u32::BITS)).expect("a word");
        let takes = |op: Op, v: u32| op.form(Size::B32, &[Opnd::Reg(dst), Opnd::Imm(i64::from(v))]).is_some();
        if takes(Op::Mov, bits) {
            self.emit(Op::Mov, Size::B32, &[dst.into(), i64::from(bits).into()]);
        } else if takes(Op::Mvn, !bits) {
            self.emit(Op::Mvn, Size::B32, &[dst.into(), i64::from(!bits).into()]);
        } else {
            self.emit(Op::Movw, Size::B32, &[dst.into(), i64::from(bits & 0xffff).into()]);
            self.emit(Op::Movt, Size::B32, &[dst.into(), i64::from(bits >> HALF).into()]);
        }
    }

    fn offset(&mut self, dst: Reg, base: Reg, disp: i64) {
        let takes =
            |op: Op, n: i64| op.form(Size::B32, &[Opnd::Reg(dst), Opnd::Reg(base), Opnd::Imm(n)]).is_some();
        if disp == 0 {
            Thumb2::copy(self, dst, base);
        } else if takes(Op::Add, disp) {
            self.emit(Op::Add, Size::B32, &[dst.into(), base.into(), disp.into()]);
        } else if takes(Op::Sub, disp.wrapping_neg()) {
            self.emit(Op::Sub, Size::B32, &[dst.into(), base.into(), disp.wrapping_neg().into()]);
        } else {
            let scratch = if dst == base || dst == Reg::Sp { FAR } else { dst };
            self.constant(scratch, disp.cast_unsigned());
            if dst == Reg::Sp {
                // The stack pointer is moved by the form adding a register to it.
                self.emit(Op::Add, Size::B32, &[dst.into(), scratch.into()]);
            } else {
                self.emit(Op::Add, Size::B32, &[dst.into(), base.into(), scratch.into()]);
            }
        }
    }

    fn address(&mut self, dst: Reg, target: Address) {
        self.emit(Op::Movw, Size::B32, &[dst.into(), Opnd::Low(target)]);
        self.emit(Op::Movt, Size::B32, &[dst.into(), Opnd::High(target)]);
    }

    fn access(&mut self, op: Op, size: Size, reg: Opnd, base: Reg, disp: i64, scratch: Reg) {
        if op.form(size, &[reg, Opnd::Mem(Mem::at(base, disp))]).is_some() {
            self.emit(op, size, &[reg, Mem::at(base, disp).into()]);
            return;
        }
        self.constant(scratch, disp.cast_unsigned());
        let indexed = [reg, Opnd::Mem(Mem::index(base, scratch))];
        if op.form(size, &indexed).is_some() {
            self.emit(op, size, &indexed);
        } else {
            self.emit(Op::Add, Size::B32, &[scratch.into(), base.into(), scratch.into()]);
            self.emit(op, size, &[reg, Mem::at(scratch, 0).into()]);
        }
    }

    fn frame_size(&mut self, dst: Reg, size: u64) {
        let negated = u32::try_from(size).expect("frames fit 2 GiB").wrapping_neg();
        self.emit(Op::Movw, Size::B32, &[dst.into(), i64::from(negated & 0xffff).into()]);
        self.emit(Op::Movt, Size::B32, &[dst.into(), i64::from(negated >> HALF).into()]);
    }
}
