//! Floats (spec 04, section 2): held as their bits in general registers, like integers; the
//! operations are snippets through `xmm0` and `xmm1` (`select`), but for conversions between
//! floats and unsigned 64-bit integers, written here, which branch at `2^63`.

use vun_asm_x86_64::{Cc, Emit, Op, Operand, Reg, Size, Xmm};
use vun_codegen::Arch;
use vun_lir::Value;

use super::Sel;
use crate::x86_64::X86;

const X0: Xmm = Xmm(0);
const X1: Xmm = Xmm(1);

/// `2^63` in each precision, by the size of its bits: the first value past `i64`, which
/// unsigned conversions split at.
const fn two_to_63(precision: Size) -> i64 {
    match precision {
        Size::B32 => 0x5f00_0000,
        _ => 0x43e0_0000_0000_0000,
    }
}

impl Sel<'_, '_> {
    /// The unsigned `u64` `value` rounded to precision `p`: past `i64`, half of it with the low
    /// bit kept for rounding, converted and doubled.
    pub(super) fn unsigned_to_float(&mut self, value: Value, p: Size, out: Option<Value>) {
        let convert = if p == Size::B32 { Op::Cvtsi2ss } else { Op::Cvtsi2sd };
        let (n, half) = (X86::CARRIER, X86::TEMP);
        self.reg_to(value, n);
        let (big, done) = (self.asm.label(), self.asm.label());
        self.asm.emit(Op::Test, Size::B64, &[n.into(), n.into()]);
        self.asm.emit(Op::J, Size::B64, &[Cc::L.into(), big.into()]);
        self.asm.emit(convert, Size::B64, &[X0.into(), n.into()]);
        self.asm.emit(Op::Jmp, Size::B64, &[done.into()]);
        self.asm.bind(big);
        self.asm.emit(Op::Mov, Size::B64, &[half.into(), n.into()]);
        self.asm.emit(Op::Shr, Size::B64, &[half.into(), Operand::Imm(1)]);
        self.asm.emit(Op::And, Size::B64, &[n.into(), Operand::Imm(1)]);
        self.asm.emit(Op::Or, Size::B64, &[half.into(), n.into()]);
        self.asm.emit(convert, Size::B64, &[X0.into(), half.into()]);
        self.asm.emit(Op::Adds, p, &[X0.into(), X0.into()]);
        self.asm.bind(done);
        self.result_from_sse(out, p);
    }

    /// `value`, a float of precision `p`, truncated to a `u64`: from `2^63` on, converted less
    /// `2^63` with the top bit set back.
    pub(super) fn float_to_unsigned(&mut self, value: Value, p: Size, out: Option<Value>) {
        let truncate = if p == Size::B32 { Op::Cvttss2si } else { Op::Cvttsd2si };
        self.reg_to(value, X86::CARRIER);
        self.asm.emit(Op::Movd, p, &[X0.into(), X86::CARRIER.into()]);
        self.asm.emit(Op::Mov, Size::B64, &[X86::CARRIER.into(), Operand::Imm(two_to_63(p))]);
        self.asm.emit(Op::Movd, p, &[X1.into(), X86::CARRIER.into()]);
        let Some(out) = out else { return };
        let d = self.def(out);
        let (big, done) = (self.asm.label(), self.asm.label());
        self.asm.emit(Op::Ucomis, p, &[X0.into(), X1.into()]);
        self.asm.emit(Op::J, Size::B64, &[Cc::Ae.into(), big.into()]);
        self.asm.emit(truncate, Size::B64, &[d.into(), X0.into()]);
        self.asm.emit(Op::Jmp, Size::B64, &[done.into()]);
        self.asm.bind(big);
        self.asm.emit(Op::Subs, p, &[X0.into(), X1.into()]);
        self.asm.emit(truncate, Size::B64, &[d.into(), X0.into()]);
        self.asm.emit(Op::Mov, Size::B64, &[X86::CARRIER.into(), Operand::Imm(i64::MIN)]);
        self.asm.emit(Op::Xor, Size::B64, &[d.into(), X86::CARRIER.into()]);
        self.asm.bind(done);
    }

    /// Defines `out` as the float of precision `p` in `xmm0`.
    fn result_from_sse(&mut self, out: Option<Value>, p: Size) {
        if let Some(out) = out {
            let d: Reg = self.def(out);
            self.asm.emit(Op::Movd, p, &[d.into(), X0.into()]);
        }
    }
}
