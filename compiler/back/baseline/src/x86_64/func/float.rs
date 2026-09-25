//! Floats (spec 04, section 2): held as their bits in general registers, like integers, so moves,
//! spills and selects need nothing of their own; arithmetic, comparisons and conversions move
//! the bits into SSE registers and back.

use vun_backend::{Lack, Missing};
use vun_lir::{BinOp, CastOp, Cond, Scalar, Value};

use super::super::encode::{Alu, Arith, Shift};
use super::super::{Cc, Precision, Reg, Rm, Size, Xmm};
use super::FnCompiler;

/// The registers a float operation's operands are moved into.
const X: Xmm = Xmm::OPERANDS[0];
const Y: Xmm = Xmm::OPERANDS[1];
/// `2^63` in each precision: the first value past `i64`, which unsigned conversions split at.
const TWO_TO_63: [(Precision, u64); 2] =
    [(Precision::Single, 0x5f00_0000), (Precision::Double, 0x43e0_0000_0000_0000)];

impl FnCompiler<'_> {
    /// The precision of the float scalar `scalar`, or what the backend lacks for it.
    fn precision(&self, scalar: Scalar) -> Result<Precision, Lack> {
        Precision::of(scalar)
            .ok_or_else(|| self.lack(if scalar == Scalar::F16 { Missing::Half } else { Missing::Float }))
    }

    /// Moves the bits of `v`, a float of precision `p`, into `x`.
    fn load_float(&mut self, v: Value, p: Precision, x: Xmm) {
        let r = self.get(v);
        self.asm.to_xmm(p.size(), x, r);
    }

    pub(super) fn float_binary(&mut self, op: BinOp, a: Value, b: Value, out: Value) -> Result<(), Lack> {
        let p = self.precision(self.f.ty(out))?;
        let arith = match op {
            BinOp::FAdd => Arith::Add,
            BinOp::FSub => Arith::Sub,
            BinOp::FMul => Arith::Mul,
            _ => Arith::Div,
        };
        self.load_float(a, p, X);
        self.load_float(b, p, Y);
        self.asm.arith(arith, p, X, Y);
        let d = self.def(out);
        self.asm.from_xmm(p.size(), d, X);
        Ok(())
    }

    /// A float comparison, ordered as LIR's are: false where either operand is a NaN. Unordered
    /// operands set the zero, parity and carry flags: `a < b` is tested as `b > a`, above, which
    /// fails where unordered, and (in)equality also needs the parity clear.
    pub(super) fn float_compare(&mut self, cond: Cond, a: Value, b: Value, out: Value) -> Result<(), Lack> {
        let precision = self.precision(self.f.ty(a))?;
        let (left, right, cc) = match cond {
            Cond::FLt => (b, a, Cc::A),
            Cond::FLe => (b, a, Cc::Ae),
            Cond::FGt => (a, b, Cc::A),
            Cond::FGe => (a, b, Cc::Ae),
            _ => (a, b, Cc::E),
        };
        self.load_float(left, precision, X);
        self.load_float(right, precision, Y);
        self.asm.ucomis(precision, X, Y);
        let d = self.def(out);
        let ordered = match cond {
            Cond::FEq => Some((Cc::E, Cc::Np, Alu::And)),
            Cond::FNe => Some((Cc::Ne, Cc::Np, Alu::And)),
            _ => None,
        };
        match ordered {
            Some((holds, parity, join)) => {
                self.asm.setcc(holds, d);
                self.asm.setcc(parity, Reg::SCRATCH);
                self.asm.alu(join, Size::B8, d, Reg::SCRATCH);
            }
            None => self.asm.setcc(cc, d),
        }
        Ok(())
    }

    /// Conversions between floats and between floats and integers; a narrower integer result is
    /// the low bits of the 64-bit one.
    pub(super) fn float_cast(
        &mut self,
        op: CastOp,
        value: Value,
        to: Scalar,
        out: Value,
    ) -> Result<(), Lack> {
        let from = self.f.ty(value);
        match op {
            CastOp::FConvert => {
                let (source, target) = (self.precision(from)?, self.precision(to)?);
                self.load_float(value, source, X);
                self.asm.convert(source, X, X);
                let d = self.def(out);
                self.asm.from_xmm(target.size(), d, X);
            }
            CastOp::SToF | CastOp::UToF => {
                let (target, size) = (self.precision(to)?, self.size(value)?);
                let r = self.get(value);
                if op == CastOp::SToF {
                    self.asm.sign_extend(size, Reg::SCRATCH, r);
                } else {
                    self.asm.zero_extend(size, Reg::SCRATCH, r);
                }
                if op == CastOp::UToF && size == Size::B64 {
                    self.unsigned_to_float(target);
                } else {
                    self.asm.from_int(target, Size::B64, X, Reg::SCRATCH);
                }
                let d = self.def(out);
                self.asm.from_xmm(target.size(), d, X);
            }
            _ => {
                let (source, size) = (self.precision(from)?, self.size_of(to)?);
                self.load_float(value, source, X);
                let d = self.def(out);
                if op == CastOp::FToU && size == Size::B64 {
                    self.float_to_unsigned(source, d);
                } else {
                    self.asm.to_int(source, Size::B64, d, X);
                }
            }
        }
        Ok(())
    }

    /// `X` = the unsigned `u64` in `SCRATCH`, rounded to `p`: past `i64`, half of it with the
    /// low bit kept for rounding, converted and doubled.
    fn unsigned_to_float(&mut self, p: Precision) {
        let (big, done) = (self.asm.label(), self.asm.label());
        self.asm.test(Size::B64, Reg::SCRATCH, Reg::SCRATCH);
        self.asm.jcc(Cc::L, big);
        self.asm.from_int(p, Size::B64, X, Reg::SCRATCH);
        self.asm.jmp(done);
        self.asm.bind(big);
        self.asm.mov_rr(Reg::TEMP, Reg::SCRATCH);
        self.asm.shift_imm(Shift::Shr, Size::B64, Reg::TEMP, 1);
        self.asm.alu_imm(Alu::And, Size::B64, Rm::Reg(Reg::SCRATCH), 1);
        self.asm.alu(Alu::Or, Size::B64, Reg::TEMP, Reg::SCRATCH);
        self.asm.from_int(p, Size::B64, X, Reg::TEMP);
        self.asm.arith(Arith::Add, p, X, X);
        self.asm.bind(done);
    }

    /// `d` = `X`, of precision `p`, truncated to a `u64`: from `2^63` on, converted less `2^63`
    /// with the top bit set back.
    fn float_to_unsigned(&mut self, p: Precision, d: Reg) {
        let two_to_63 = TWO_TO_63.iter().find(|(q, _)| *q == p).map_or(0, |&(_, bits)| bits);
        self.asm.mov_ri(Reg::SCRATCH, two_to_63);
        self.asm.to_xmm(p.size(), Y, Reg::SCRATCH);
        let (big, done) = (self.asm.label(), self.asm.label());
        self.asm.ucomis(p, X, Y);
        self.asm.jcc(Cc::Ae, big);
        self.asm.to_int(p, Size::B64, d, X);
        self.asm.jmp(done);
        self.asm.bind(big);
        self.asm.arith(Arith::Sub, p, X, Y);
        self.asm.to_int(p, Size::B64, d, X);
        self.asm.mov_ri(Reg::SCRATCH, 1 << 63);
        self.asm.alu(Alu::Xor, Size::B64, d, Reg::SCRATCH);
        self.asm.bind(done);
    }
}
