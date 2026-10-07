//! The instructions arm64's terminators are written with (`Terminates`): a condition tested as a
//! byte, a switch's narrow value zero-extended to 32 bits, cases compared as immediates where one
//! fits, and an epilogue popping the frame pointer and the link register and returning.

use vun_asm_arm64::{Cc, Emit, Op, Operand, Reg, Size};
use vun_backend::Lack;
use vun_codegen::{Arch, Cx};
use vun_dwarf::Step;
use vun_lir::{Cond, Value};
use vun_native::select::Terminates;

use super::narrow;
use super::select::Look;
use crate::asm::AsmExt;
use crate::{Arm, FP, LR, Opnd, condition};

/// The bytes the prologue pushed, popped by the epilogue.
const PAIR: i64 = 16;
/// The byte a boolean is.
const BYTE: i64 = 0xff;

impl Arm {
    /// The registers the function saved restored, then `mov sp, x29; ldp x29, x30, [sp], 16`.
    pub(super) fn leave(cx: &mut Cx<'_, Self>) {
        cx.restore();
        cx.unwound(Step::Leaving);
        cx.asm.emit(Op::Mov, Size::B64, &[Reg::Sp.into(), FP.into()]);
        let popped = vun_asm_arm64::Mem::Post { base: Reg::Sp, offset: PAIR };
        cx.asm.emit(Op::Ldp, Size::B64, &[FP.into(), LR.into(), popped.into()]);
        cx.unwound(Step::Popped);
    }
}

impl Terminates for Arm {
    const EQUAL: Cc = Cc::Eq;

    fn holds(cond: Cond) -> Option<Cc> {
        condition(cond)
    }

    fn inverse(cond: Cc) -> Cc {
        cond.not()
    }

    fn test(cx: &mut Cx<'_, Self>, r: Reg) -> Cc {
        cx.asm.emit(Op::Tst, Size::B32, &[r.into(), Operand::Imm(BYTE)]);
        Cc::Ne
    }

    fn switched(cx: &mut Cx<'_, Self>, v: Value) -> Result<(Reg, Size), Lack> {
        let (r, size) = (cx.reg(v), Look(cx).size(v)?);
        if !narrow(size) {
            return Ok((r, size));
        }
        let extended = cx.scratch();
        cx.asm.emit(Op::Uxt, size, &[extended.into(), r.into()]);
        Ok((extended, Size::B32))
    }

    /// `cmp r, bits`, at `size`: as an immediate where it fits one, else through the carrier.
    fn compare_case(cx: &mut Cx<'_, Self>, r: Reg, size: Size, bits: u128) {
        let value = vun_asm::imm::low64(bits).cast_signed();
        let operands: [Opnd; 2] = [r.into(), Operand::Imm(value)];
        if Op::Cmp.form(size, &operands).is_none() {
            cx.asm.constant(Self::CARRIER, value.cast_unsigned());
            cx.asm.emit(Op::Cmp, size, &[r.into(), Self::CARRIER.into()]);
        } else {
            cx.asm.emit(Op::Cmp, size, &operands);
        }
    }

    fn branch_if(cx: &mut Cx<'_, Self>, cond: Cc, label: vun_asm::Label) {
        cx.asm.emit(Op::Bcond, Size::B64, &[cond.into(), label.into()]);
    }

    fn trap(cx: &mut Cx<'_, Self>) {
        cx.asm.emit(Op::Udf, Size::B64, &[Operand::Imm(0)]);
    }

    /// Leaves the frame, then `ret`.
    fn epilogue(cx: &mut Cx<'_, Self>) {
        Self::leave(cx);
        cx.asm.emit(Op::Ret, Size::B64, &[]);
        cx.unwound(Step::Returned);
    }
}
