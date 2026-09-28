//! Doubles, which the floating-point extension computes but no general register holds: each
//! lives in its home, two words (`Arch::bank`), and each operation loads its operands into `d0`
//! and `d1`, computes there, and stores its result to its home, or moves it into a general
//! register where it is a single or an integer. Nothing holds a double between instructions, as
//! nothing holds a single.

use vun_asm_thumb::{Cc, Emit, Mem, Op, Operand, Size, V};
use vun_backend::{Lack, Missing};
use vun_codegen::snippet::Args;
use vun_lir::{BinOp, CastOp, Inst, InstKind, Scalar, Value};

use super::Sel;
use crate::asm::AsmExt;
use crate::snippets::FLAGS_SET;
use crate::{FAR, FP, Flags, Instr, Thumb2};
use vun_codegen::Arch;

/// The registers a double is computed in: `d0` and `d1`, which are `v0` and `v2` at their width.
const D0: V = V::V0;
const D1: V = V::V2;
/// A single's register, `s0`, the low half of `d0`.
const S0: V = V::V0;
/// The byte a boolean is.
const BYTE: i64 = 0xff;

impl Sel<'_, '_> {
    /// `inst`, which reads or gives a double.
    pub(super) fn double(&mut self, inst: Inst) -> Result<(), Lack> {
        let f = self.f;
        let data = &f.insts[inst];
        let out = data.result;
        let result = || out.expect("an operation giving a value");
        match data.kind {
            InstKind::Const { bits, .. } => self.constant_home(result(), bits),
            InstKind::Binary { op, a, b } => {
                let op = match op {
                    BinOp::FAdd => Op::Vadd,
                    BinOp::FSub => Op::Vsub,
                    BinOp::FMul => Op::Vmul,
                    BinOp::FDiv => Op::Vdiv,
                    _ => return Err(self.lack(Missing::Wider(8))),
                };
                self.load_double(D0, a);
                self.load_double(D1, b);
                self.asm.emit(op, Size::B64, &[D0.into(), D0.into(), D1.into()]);
                self.store_double(result(), D0);
            }
            InstKind::Compare { cond, a, b } => {
                let holds = cond.holds().ok_or_else(|| self.lack(Missing::Wider(8)))?;
                self.load_double(D0, a);
                self.load_double(D1, b);
                self.asm.emit(Op::Vcmp, Size::B64, &[D0.into(), D1.into()]);
                let out = Some(result());
                self.snippet(FLAGS_SET, &Args { ins: &[], out, size: Size::B32, cond: Some(holds) });
            }
            InstKind::Cast { op, value, .. } => self.converted(op, value, result())?,
            InstKind::Select { cond, then, otherwise } => {
                let c = self.reg(cond);
                let skip = self.asm.label();
                self.load_double(D0, otherwise);
                self.asm.emit(Op::Tst, Size::B32, &[c.into(), Operand::Imm(BYTE)]);
                self.asm.emit(Op::Bcond, Size::B32, &[Cc::Eq.into(), skip.into()]);
                self.load_double(D0, then);
                self.asm.bind(skip);
                self.store_double(result(), D0);
            }
            InstKind::Load { addr, .. } => {
                let at = self.reg(addr);
                self.asm.emit(Op::Vldr, Size::B64, &[D0.into(), Mem::at(at, 0).into()]);
                self.store_double(result(), D0);
            }
            InstKind::Store { value, addr, .. } => {
                let at = self.reg(addr);
                self.load_double(D0, value);
                self.asm.emit(Op::Vstr, Size::B64, &[D0.into(), Mem::at(at, 0).into()]);
            }
            _ => return Err(self.lack(Missing::Wider(8))),
        }
        Ok(())
    }

    /// A conversion from or to a double, giving `out`: between the precisions, with an integer of
    /// a word at most.
    fn converted(&mut self, op: CastOp, value: Value, out: Value) -> Result<(), Lack> {
        let from = self.f.ty(value);
        match (op, from == Scalar::F64) {
            // Into a double: a single or an integer moved into `s0`, extended first where narrow.
            (CastOp::FConvert | CastOp::SToF | CastOp::UToF, false) => {
                let r = self.reg(value);
                let size = self.size_of(from)?;
                let bits = if size.narrow() {
                    let extend = if op == CastOp::SToF { Instr::Sext } else { Instr::Zext };
                    let (extend, at) = extend.at(size);
                    self.asm.emit(extend, at, &[Thumb2::CARRIER.into(), r.into()]);
                    Thumb2::CARRIER
                } else {
                    r
                };
                self.asm.emit(Op::Vmov, Size::B32, &[S0.into(), bits.into()]);
                let convert = match op {
                    CastOp::FConvert => Op::VcvtPrecision,
                    CastOp::SToF => Op::VcvtFromSigned,
                    _ => Op::VcvtFromUnsigned,
                };
                self.asm.emit(convert, Size::B64, &[D0.into(), S0.into()]);
                self.store_double(out, D0);
            }
            // Out of a double: into `s0`, then its bits or its integer into the result's register.
            (CastOp::FConvert | CastOp::FToS | CastOp::FToU, true) => {
                self.load_double(D0, value);
                let (convert, size) = match op {
                    CastOp::FConvert => (Op::VcvtPrecision, Size::B32),
                    CastOp::FToS => (Op::VcvtSigned, Size::B64),
                    _ => (Op::VcvtUnsigned, Size::B64),
                };
                self.asm.emit(convert, size, &[S0.into(), D0.into()]);
                let d = self.def(out);
                self.asm.emit(Op::Vmov, Size::B32, &[d.into(), S0.into()]);
            }
            _ => return Err(self.lack(Missing::Wider(8))),
        }
        Ok(())
    }

    /// Writes the double of `bits` into the home of `v`, a word at a time.
    fn constant_home(&mut self, v: Value, bits: u128) {
        let home = self.home(v);
        for (word, at) in [(bits, 0), (bits >> 32, 4)] {
            self.asm.constant(Thumb2::CARRIER, vun_asm::imm::low64(word) & u64::from(u32::MAX));
            Thumb2::store(&mut self.asm, home + at, Thumb2::CARRIER);
        }
    }

    /// Loads the double `v` from its home into `d`.
    pub(super) fn load_double(&mut self, d: V, v: Value) {
        let home = i64::from(self.home(v));
        self.asm.access(Op::Vldr, Size::B64, d.into(), FP, home, FAR);
    }

    /// Stores `d` into the home of the double `v`.
    pub(super) fn store_double(&mut self, v: Value, d: V) {
        let home = i64::from(self.home(v));
        self.asm.access(Op::Vstr, Size::B64, d.into(), FP, home, FAR);
    }
}
