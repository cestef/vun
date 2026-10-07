//! The instructions arm64's calls, parameters and returns are written with (`Frames`).

use vun_asm_arm64::{Emit, Op, Operand, Reg, Size, V};
use vun_backend::Lack;
use vun_codegen::{Arch, Cx};
use vun_dwarf::Step;
use vun_lir::{Address, Callee, Ext, Scalar, Value};
use vun_native::select::Frames;

use super::Sel;
use super::entry::CALLER_ARGS;
use super::select::Look;
use crate::Arm;
use crate::abi::Carrier;

impl Frames for Arm {
    const CALLER_ARGS: i32 = CALLER_ARGS;
    const STACK: Reg = Reg::Sp;

    fn sized(cx: &Cx<'_, Self>, scalar: Scalar) -> Result<Size, Lack> {
        Look(cx).size_of(scalar)
    }

    fn pass(cx: &mut Cx<'_, Self>, v: Value, carrier: Carrier) {
        Sel(cx).pass(v, carrier);
    }

    fn receive_float(cx: &mut Cx<'_, Self>, v: Value, x: V) {
        Sel(cx).receive_float(v, x);
    }

    fn extend(cx: &mut Cx<'_, Self>, r: Reg, size: Size, ext: Ext) {
        Sel(cx).extend(r, size, ext);
    }

    fn outgoing(cx: &mut Cx<'_, Self>, a: Value, offset: u64, bytes: u64) {
        cx.reg_to(a, Self::CARRIER);
        Self::store_outgoing(cx, Self::CARRIER, offset, bytes);
    }

    fn branch_link(cx: &mut Cx<'_, Self>, callee: &Callee) {
        match callee {
            Callee::Func(f) => cx.asm.emit(Op::Bl, Size::B64, &[Operand::Sym(Address::Func(*f))]),
            Callee::Import(i) => cx.asm.emit(Op::Bl, Size::B64, &[Operand::Sym(Address::Import(*i))]),
            Callee::Indirect(target, _) => {
                cx.reg_to(*target, Self::CARRIER);
                cx.asm.emit(Op::Blr, Size::B64, &[Self::CARRIER.into()]);
            }
        }
    }

    fn branch_tail(cx: &mut Cx<'_, Self>, callee: &Callee) -> Result<(), Lack> {
        // The target waits in the carrier, which leaving the frame leaves alone.
        if let Callee::Indirect(target, _) = callee {
            cx.reg_to(*target, Self::CARRIER);
        }
        Self::leave(cx);
        match callee {
            Callee::Func(f) => cx.asm.emit(Op::BAlways, Size::B64, &[Operand::Sym(Address::Func(*f))]),
            Callee::Import(i) => cx.asm.emit(Op::BAlways, Size::B64, &[Operand::Sym(Address::Import(*i))]),
            Callee::Indirect(..) => cx.asm.emit(Op::Br, Size::B64, &[Self::CARRIER.into()]),
        }
        cx.unwound(Step::Returned);
        Ok(())
    }

    fn gathered(cx: &Cx<'_, Self>) -> Option<i32> {
        cx.state.gathered
    }

    fn gather(cx: &mut Cx<'_, Self>, at: i32) {
        cx.state.gathered = Some(at);
    }
}
