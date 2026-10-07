//! The instructions Thumb's terminators are written with (`Terminates`): a condition tested as a
//! byte, a switch's value zero-extended to a word, cases compared as immediates where one fits,
//! and an epilogue popping the return address into the program counter.

use vun_asm::imm::sign_extend;
use vun_asm_thumb::{Cc, Emit, List, Op, Operand, Reg, Size};
use vun_backend::Lack;
use vun_codegen::{Arch, Cx};
use vun_dwarf::Step;
use vun_lir::{Cond, Value};
use vun_native::select::Terminates;

use super::entry::RETURN;
use super::select::Look;
use crate::asm::AsmExt;
use crate::{FAR, FP, Flags, Instr, Opnd, Thumb2};

/// The byte a boolean is.
const BYTE: i64 = 0xff;

impl Thumb2 {
    /// `FAR` restored with the registers the function saved, then `mov sp, r7; pop` of `popped`.
    pub(super) fn leave(cx: &mut Cx<'_, Self>, popped: List) {
        let far = cx.state.far_at;
        Self::load(&mut cx.asm, FAR, far);
        cx.restore();
        cx.unwound(Step::Leaving);
        cx.asm.emit(Op::Mov, Size::B32, &[Reg::Sp.into(), FP.into()]);
        cx.asm.emit(Op::Pop, Size::B32, &[Operand::List(popped)]);
        cx.unwound(Step::Popped);
    }
}

impl Terminates for Thumb2 {
    const EQUAL: Cc = Cc::Eq;

    fn holds(cond: Cond) -> Option<Cc> {
        cond.holds()
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
        if !size.narrow() {
            return Ok((r, Size::B32));
        }
        let extended = cx.scratch();
        let (op, size) = Instr::Zext.at(size);
        cx.asm.emit(op, size, &[extended.into(), r.into()]);
        Ok((extended, Size::B32))
    }

    /// `cmp r, bits`: as an immediate where it fits one, else through the carrier.
    fn compare_case(cx: &mut Cx<'_, Self>, r: Reg, size: Size, bits: u128) {
        let value = vun_asm::imm::low64(bits);
        let operands: [Opnd; 2] = [r.into(), Operand::Imm(sign_extend(value, u32::BITS))];
        if Op::Cmp.form(size, &operands).is_none() {
            cx.asm.constant(Self::CARRIER, value);
            cx.asm.emit(Op::Cmp, size, &[r.into(), Self::CARRIER.into()]);
        } else {
            cx.asm.emit(Op::Cmp, size, &operands);
        }
    }

    fn branch_if(cx: &mut Cx<'_, Self>, cond: Cc, label: vun_asm::Label) {
        cx.asm.emit(Op::Bcond, Size::B32, &[cond.into(), label.into()]);
    }

    fn trap(cx: &mut Cx<'_, Self>) {
        cx.asm.emit(Op::Udf, Size::B32, &[Operand::Imm(0)]);
    }

    /// Leaves the frame, returning through `pc`.
    fn epilogue(cx: &mut Cx<'_, Self>) {
        Self::leave(cx, RETURN);
        cx.unwound(Step::Returned);
    }
}
