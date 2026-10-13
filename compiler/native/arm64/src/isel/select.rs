//! How each instruction is compiled, decided once: the snippet it is, over which operands, at
//! which size, or a sequence written here. What it needs of the plan and what it emits both
//! read the same decision.

use vun_asm_arm64::Size;
use vun_backend::{Lack, Missing};
use vun_codegen::Cx;
use vun_codegen::snippet::Snippet;
use vun_lir::{BinOp, CastOp, Cond, Inst, InstKind, Reads, Scalar, Value};

use crate::snippets::{
    ADD, AND, CMP, CMP_SIGNED, CMP_UNSIGNED, DOUBLE_TO_SIGNED, DOUBLE_TO_UNSIGNED, FADD, FCMP, FDIV, FMUL,
    FNE, FSET, FSUB, KEEP, LOAD, LOAD_ACQUIRE, MUL, NARROW, NARROW_SIGNED_TO_DOUBLE, NARROW_SIGNED_TO_SINGLE,
    NARROW_UNSIGNED_TO_DOUBLE, NARROW_UNSIGNED_TO_SINGLE, OR, SAR, SAR_NARROW, SDIV, SDIV_NARROW, SELECT,
    SET, SET_SIGNED, SET_UNSIGNED, SEXT, SHL, SHR, SHR_NARROW, SIGNED_TO_DOUBLE, SIGNED_TO_SINGLE,
    SINGLE_TO_SIGNED, SINGLE_TO_UNSIGNED, SREM, SREM_NARROW, STORE, STORE_RELEASE, SUB, UDIV, UDIV_NARROW,
    UNSIGNED_TO_DOUBLE, UNSIGNED_TO_SINGLE, UREM, UREM_NARROW, WIDEN, XOR, ZEXT,
};
use crate::{Arm, condition};
use vun_native::inline::Inlined;
use vun_native::width::size_of;

/// An instruction as it is compiled.
pub(super) type Selection<'f> = vun_native::select::Selection<'f, Arm>;

/// The function compiled, read without emitting.
pub(super) struct Look<'c, 'a>(pub(super) &'c Cx<'a, Arm>);

impl<'a> Look<'_, 'a> {
    /// The size of an integer, address or float value, or what the backend lacks for it.
    pub(super) fn size(&self, v: Value) -> Result<Size, Lack> {
        self.size_of(self.0.f.ty(v))
    }

    pub(super) fn size_of(&self, scalar: Scalar) -> Result<Size, Lack> {
        size_of(scalar, self.0.unit.pointer_bits).ok_or_else(|| {
            self.0.lack(match scalar {
                Scalar::I128 => Missing::Int128,
                _ => Missing::Half,
            })
        })
    }

    /// The size of the bits of the float scalar `scalar`, which is its precision.
    fn precision(&self, scalar: Scalar) -> Result<Size, Lack> {
        match scalar {
            Scalar::F32 => Ok(Size::B32),
            Scalar::F64 => Ok(Size::B64),
            _ => Err(self.0.lack(Missing::Half)),
        }
    }

    /// How `inst` is compiled.
    pub(super) fn selection(&self, inst: Inst) -> Result<Selection<'a>, Lack> {
        let f = self.0.f;
        let snippet = |snippet, ins: &[Value], size, cond| Selection::Snippet {
            snippet,
            ins: ins.to_vec(),
            size,
            cond,
        };
        Ok(match &f.insts[inst].kind {
            &InstKind::Binary { op, a, b } if op.reads() == Reads::Float => {
                let chosen = match op {
                    BinOp::FAdd => FADD,
                    BinOp::FSub => FSUB,
                    BinOp::FMul => FMUL,
                    _ => FDIV,
                };
                snippet(chosen, &[a, b], self.precision(f.ty(a))?, None)
            }
            &InstKind::Binary { op, a, b } => {
                let size = self.size(a)?;
                let chosen = match (op, size.narrow()) {
                    (BinOp::Add, _) => ADD,
                    (BinOp::Sub, _) => SUB,
                    (BinOp::And, _) => AND,
                    (BinOp::Or, _) => OR,
                    (BinOp::Xor, _) => XOR,
                    (BinOp::Mul, _) => MUL,
                    (BinOp::Shl, _) => SHL,
                    (BinOp::UDiv, false) => UDIV,
                    (BinOp::URem, false) => UREM,
                    (BinOp::SDiv, false) => SDIV,
                    (BinOp::SRem, false) => SREM,
                    (BinOp::LShr, false) => SHR,
                    (BinOp::AShr, false) => SAR,
                    (BinOp::UDiv, true) => UDIV_NARROW,
                    (BinOp::URem, true) => UREM_NARROW,
                    (BinOp::SDiv, true) => SDIV_NARROW,
                    (BinOp::SRem, true) => SREM_NARROW,
                    (BinOp::LShr, true) => SHR_NARROW,
                    (BinOp::AShr, true) => SAR_NARROW,
                    (BinOp::FAdd | BinOp::FSub | BinOp::FMul | BinOp::FDiv, _) => {
                        unreachable!("floats are read as floats")
                    }
                };
                snippet(chosen, &[a, b], size, None)
            }
            &InstKind::Compare { cond, a, b } if cond.reads() == Reads::Float => {
                // Ordered, as LIR's comparisons are: false where either operand is a NaN.
                let precision = self.precision(f.ty(a))?;
                match condition(cond) {
                    Some(cc) => snippet(FSET, &[a, b], precision, Some(cc)),
                    None => snippet(FNE, &[a, b], precision, None),
                }
            }
            &InstKind::Compare { cond, a, b } => {
                let size = self.size(a)?;
                let chosen = match (size.narrow(), cond.reads()) {
                    (false, _) => SET,
                    (true, Reads::Signed) => SET_SIGNED,
                    (true, _) => SET_UNSIGNED,
                };
                snippet(chosen, &[a, b], size, condition(cond))
            }
            &InstKind::Cast { op, value, to } => self.cast(op, value, to)?,
            &InstKind::Select { cond, then, otherwise } => {
                self.size(then)?;
                snippet(SELECT, &[cond, then, otherwise], Size::B64, None)
            }
            InstKind::Call { callee, args, kind } => {
                for &a in args {
                    self.size(a)?;
                }
                Selection::Call { callee, args, kind: *kind }
            }
            &InstKind::Load { ty, addr, access } => {
                snippet(if access.ordered() { LOAD_ACQUIRE } else { LOAD }, &[addr], self.size_of(ty)?, None)
            }
            &InstKind::Store { value, addr, access } => snippet(
                if access.ordered() { STORE_RELEASE } else { STORE },
                &[value, addr],
                self.size(value)?,
                None,
            ),
            &InstKind::Rmw { op, addr, value, .. } => {
                Selection::Rmw { op, addr, value, size: self.size(value)? }
            }
            &InstKind::CompareExchange { addr, expected, new, .. } => {
                Selection::CompareExchange { addr, expected, new, size: self.size(new)? }
            }
            &InstKind::Offset { base, offset } => snippet(ADD, &[base, offset], Size::B64, None),
            InstKind::GrowMemory { .. } => return Err(self.0.lack(Missing::LinearMemory)),
            &InstKind::Copy { to, from, bytes } => Selection::Copy { to, from, bytes },
            InstKind::Asm(asm, check) => {
                asm.check::<Arm>()?;
                Selection::Asm(asm, check.as_ref())
            }
            &InstKind::ThreadAddr(data) => Selection::ThreadAddr { data },
            InstKind::Const { .. } | InstKind::SlotAddr(_) | InstKind::Addr(_) => {
                unreachable!("recomputed where used")
            }
        })
    }

    /// The flags a branch fusing the comparison `cond` of `a` and `b` reads, as a snippet.
    pub(super) fn flags(&self, cond: Cond, a: Value) -> Result<(Snippet<Arm>, Size), Lack> {
        let size = self.size(a)?;
        let snippet = match (cond.reads(), size.narrow()) {
            (Reads::Float, _) => FCMP,
            (_, false) => CMP,
            (Reads::Signed, true) => CMP_SIGNED,
            (_, true) => CMP_UNSIGNED,
        };
        Ok((snippet, size))
    }

    /// Conversions between integers, between floats, and between floats and integers; a
    /// narrower integer result is the low bits of the 64-bit one.
    fn cast(&self, op: CastOp, value: Value, to: Scalar) -> Result<Selection<'a>, Lack> {
        let snippet = |snippet, size| Selection::Snippet { snippet, ins: vec![value], size, cond: None };
        let from = self.0.f.ty(value);
        Ok(match op {
            CastOp::ZExt
            | CastOp::SExt
            | CastOp::Trunc
            | CastOp::PtrToInt
            | CastOp::IntToPtr
            | CastOp::Bitcast => {
                self.size_of(to)?;
                let chosen = match op {
                    CastOp::ZExt => ZEXT,
                    CastOp::SExt => SEXT,
                    _ => KEEP,
                };
                snippet(chosen, self.size(value)?)
            }
            CastOp::FConvert => {
                self.precision(from)?;
                snippet(if self.precision(to)? == Size::B64 { WIDEN } else { NARROW }, Size::B64)
            }
            CastOp::SToF | CastOp::UToF => {
                let (single, size) = (self.precision(to)? == Size::B32, self.size(value)?);
                let chosen = match (op == CastOp::SToF, size.narrow(), single) {
                    (true, false, true) => SIGNED_TO_SINGLE,
                    (true, false, false) => SIGNED_TO_DOUBLE,
                    (false, false, true) => UNSIGNED_TO_SINGLE,
                    (false, false, false) => UNSIGNED_TO_DOUBLE,
                    (true, true, true) => NARROW_SIGNED_TO_SINGLE,
                    (true, true, false) => NARROW_SIGNED_TO_DOUBLE,
                    (false, true, true) => NARROW_UNSIGNED_TO_SINGLE,
                    (false, true, false) => NARROW_UNSIGNED_TO_DOUBLE,
                };
                snippet(chosen, size)
            }
            CastOp::FToS | CastOp::FToU => {
                let precision = self.precision(from)?;
                self.size_of(to)?;
                let chosen = match (op == CastOp::FToS, precision == Size::B32) {
                    (true, true) => SINGLE_TO_SIGNED,
                    (true, false) => DOUBLE_TO_SIGNED,
                    (false, true) => SINGLE_TO_UNSIGNED,
                    (false, false) => DOUBLE_TO_UNSIGNED,
                };
                snippet(chosen, precision)
            }
        })
    }
}
