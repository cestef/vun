//! SSE instructions: float arithmetic, comparisons and conversions, and moves between general
//! and SSE registers. Each has its mandatory prefix before REX, then `0F` and its opcode.

use super::super::{Precision, Reg, Rm, Size, Xmm};
use super::{Asm, Bytes, Field};

/// A float arithmetic operation, by its opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arith {
    Add = 0x58,
    Mul = 0x59,
    Sub = 0x5c,
    Div = 0x5e,
}

/// The prefix of a scalar operation: `F3` single, `F2` double.
const fn scalar(precision: Precision) -> u8 {
    match precision {
        Precision::Single => 0xf3,
        Precision::Double => 0xf2,
    }
}

/// The prefix of `movd`/`movq` and of the double `ucomisd`.
const PACKED: u8 = 0x66;

impl Asm {
    /// One SSE instruction: `prefix`, REX (`W` where `wide`), `0F opcode`, `ModRM`.
    fn sse(&mut self, prefix: Option<u8>, wide: bool, opcode: u8, field: Field, rm: Rm) {
        if let Some(prefix) = prefix {
            self.bytes(&[prefix]);
        }
        let size = if wide { Size::B64 } else { Size::B32 };
        self.encode(size, Bytes { field: false, rm: false }, &[0x0f, opcode], field, rm);
    }

    /// `movd`/`movq xmm, r`: the low `size` bytes of `src` into `dst`.
    pub fn to_xmm(&mut self, size: Size, dst: Xmm, src: Reg) {
        self.sse(Some(PACKED), size == Size::B64, 0x6e, Field::Xmm(dst), Rm::Reg(src));
    }

    /// `movd`/`movq r, xmm`: the low `size` bytes of `src` into `dst`.
    pub fn from_xmm(&mut self, size: Size, dst: Reg, src: Xmm) {
        self.sse(Some(PACKED), size == Size::B64, 0x7e, Field::Xmm(src), Rm::Reg(dst));
    }

    /// `dst = dst op src`.
    pub fn arith(&mut self, op: Arith, precision: Precision, dst: Xmm, src: Xmm) {
        self.sse(Some(scalar(precision)), false, op as u8, Field::Xmm(dst), Rm::Xmm(src));
    }

    /// Sets the flags from comparing `a` with `b`, unordered where either is a NaN.
    pub fn ucomis(&mut self, precision: Precision, a: Xmm, b: Xmm) {
        let prefix = match precision {
            Precision::Single => None,
            Precision::Double => Some(PACKED),
        };
        self.sse(prefix, false, 0x2e, Field::Xmm(a), Rm::Xmm(b));
    }

    /// `dst` = `src`, of precision `from`, in the other precision.
    pub fn convert(&mut self, from: Precision, dst: Xmm, src: Xmm) {
        self.sse(Some(scalar(from)), false, 0x5a, Field::Xmm(dst), Rm::Xmm(src));
    }

    /// `dst` = the signed integer of `size` in `src`, rounded to `precision`.
    pub fn from_int(&mut self, precision: Precision, size: Size, dst: Xmm, src: Reg) {
        self.sse(Some(scalar(precision)), size == Size::B64, 0x2a, Field::Xmm(dst), Rm::Reg(src));
    }

    /// `dst` = `src` truncated toward zero, a signed integer of `size`.
    pub fn to_int(&mut self, precision: Precision, size: Size, dst: Reg, src: Xmm) {
        self.sse(Some(scalar(precision)), size == Size::B64, 0x2c, Field::Reg(dst), Rm::Xmm(src));
    }
}
