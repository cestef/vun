//! Floats (spec 04, section 2): held as their bits in general registers, like integers; the
//! operations are snippets through `xmm0` and `xmm1`, but for conversions between floats and
//! unsigned 64-bit integers, which branch at `2^63`.

use vun_asm_x86_64::{Cc, Emit, Op, Operand, Reg, Size, Xmm};
use vun_backend::{Lack, Missing};
use vun_codegen::Arch;
use vun_lir::{BinOp, CastOp, Cond, Scalar, Value};

use super::Sel;
use crate::x86_64::X86;
use crate::x86_64::snippets::{
    DOUBLE_TO_INT, FADD, FCOMPARE, FDIV, FEQUAL, FMUL, FSUB, NARROW, SIGNED_TO_DOUBLE, SIGNED_TO_SINGLE,
    SINGLE_TO_INT, UNSIGNED_TO_DOUBLE, UNSIGNED_TO_SINGLE, WIDEN,
};

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
    /// The size of the bits of the float scalar `scalar`, which is its precision.
    fn precision(&self, scalar: Scalar) -> Result<Size, Lack> {
        match scalar {
            Scalar::F32 => Ok(Size::B32),
            Scalar::F64 => Ok(Size::B64),
            _ => Err(self.lack(Missing::Half)),
        }
    }

    pub(super) fn float_binary(
        &mut self,
        op: BinOp,
        a: Value,
        b: Value,
        out: Option<Value>,
    ) -> Result<(), Lack> {
        let snippet = match op {
            BinOp::FAdd => FADD,
            BinOp::FSub => FSUB,
            BinOp::FMul => FMUL,
            _ => FDIV,
        };
        let p = self.precision(self.f.ty(a))?;
        self.apply(snippet, &[a, b], out, p, None);
        Ok(())
    }

    /// A float comparison, ordered as LIR's are: false where either operand is a NaN.
    /// Unordered operands set the zero, parity and carry flags: `a < b` is tested as `b > a`,
    /// above, which fails where unordered, and (in)equality also needs the parity clear.
    pub(super) fn float_compare(
        &mut self,
        cond: Cond,
        a: Value,
        b: Value,
        out: Option<Value>,
    ) -> Result<(), Lack> {
        let p = self.precision(self.f.ty(a))?;
        let (snippet, ins, cc) = match cond {
            Cond::FLt => (FCOMPARE, [b, a], Cc::A),
            Cond::FLe => (FCOMPARE, [b, a], Cc::Ae),
            Cond::FGt => (FCOMPARE, [a, b], Cc::A),
            Cond::FGe => (FCOMPARE, [a, b], Cc::Ae),
            Cond::FEq => (FEQUAL, [a, b], Cc::E),
            _ => (FEQUAL, [a, b], Cc::Ne),
        };
        self.apply(snippet, &ins, out, p, Some(cc));
        Ok(())
    }

    /// Conversions between floats, and between floats and integers; a narrower integer result
    /// is the low bits of the 64-bit one.
    pub(super) fn float_cast(
        &mut self,
        op: CastOp,
        value: Value,
        to: Scalar,
        out: Option<Value>,
    ) -> Result<(), Lack> {
        let from = self.f.ty(value);
        match op {
            CastOp::FConvert => {
                let snippet = if self.precision(to)? == Size::B64 { WIDEN } else { NARROW };
                self.precision(from)?;
                self.apply(snippet, &[value], out, Size::B64, None);
            }
            CastOp::SToF | CastOp::UToF => {
                let (p, size) = (self.precision(to)?, self.size(value)?);
                if op == CastOp::UToF && size == Size::B64 {
                    self.unsigned_to_float(value, p, out);
                } else {
                    let snippet = match (op, p) {
                        (CastOp::SToF, Size::B32) => SIGNED_TO_SINGLE,
                        (CastOp::SToF, _) => SIGNED_TO_DOUBLE,
                        (_, Size::B32) => UNSIGNED_TO_SINGLE,
                        _ => UNSIGNED_TO_DOUBLE,
                    };
                    self.apply(snippet, &[value], out, size, None);
                }
            }
            _ => {
                let (p, size) = (self.precision(from)?, self.size_of(to)?);
                if op == CastOp::FToU && size == Size::B64 {
                    self.float_to_unsigned(value, p, out);
                } else {
                    let snippet = if p == Size::B32 { SINGLE_TO_INT } else { DOUBLE_TO_INT };
                    self.apply(snippet, &[value], out, Size::B64, None);
                }
            }
        }
        Ok(())
    }

    /// The unsigned `u64` `value` rounded to precision `p`: past `i64`, half of it with the low
    /// bit kept for rounding, converted and doubled.
    fn unsigned_to_float(&mut self, value: Value, p: Size, out: Option<Value>) {
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
    fn float_to_unsigned(&mut self, value: Value, p: Size, out: Option<Value>) {
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
