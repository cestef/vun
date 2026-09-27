//! How each instruction is compiled, decided once: the snippet it is, over which operands, at
//! which size, or a sequence written here. What it needs of the plan and what it emits both
//! read the same decision.

use vun_asm_x86_64::{Cc, Op, Size};
use vun_backend::{Lack, Missing};
use vun_codegen::Cx;
use vun_codegen::snippet::Snippet;
use vun_lir::{Access, BinOp, Callee, CastOp, Cond, Inst, InstKind, Order, Reads, RmwOp, Scalar, Value};

use crate::inline::Inlined;
use crate::width::size_of;
use crate::x86_64::snippets::{
    ADD, AND, COMPARE_EXCHANGE, DOUBLE_TO_INT, FADD, FCOMPARE, FDIV, FEQUAL, FETCH_ADD, FETCH_SUB, FMUL,
    FSUB, KEEP, LOAD, MUL, NARROW, OR, SAR, SDIV, SELECT, SET, SEXT, SHL, SHR, SIGNED_TO_DOUBLE,
    SIGNED_TO_SINGLE, SINGLE_TO_INT, SREM, STORE, STORE_SEQ_CST, SUB, SWAP, UDIV, UNSIGNED_TO_DOUBLE,
    UNSIGNED_TO_SINGLE, UREM, WIDEN, XOR, ZEXT,
};
use crate::x86_64::{X86, condition};

/// An instruction as it is compiled.
pub(super) enum Selection<'f> {
    Snippet {
        snippet: Snippet<X86>,
        ins: Vec<Value>,
        size: Size,
        cond: Option<Cc>,
    },
    Call {
        callee: &'f Callee,
        args: &'f [Value],
    },
    /// `op` on memory at `addr` with `value`, atomically, in a loop of compare-exchanges.
    Fetch {
        op: Op,
        addr: Value,
        value: Value,
        size: Size,
    },
    Copy {
        to: Value,
        from: Value,
        bytes: u64,
    },
    /// An unsigned `u64` to a float of `precision`, or back.
    UnsignedToFloat {
        value: Value,
        precision: Size,
    },
    FloatToUnsigned {
        value: Value,
        precision: Size,
    },
    Asm(&'f vun_lir::Asm),
}

/// The function compiled, read without emitting.
pub(super) struct Look<'c, 'a>(pub(super) &'c Cx<'a, X86>);

impl<'a> Look<'_, 'a> {
    /// The size of an integer, address or float value, or what the backend lacks for it.
    pub(super) fn size(&self, v: Value) -> Result<Size, Lack> {
        self.size_of(self.0.f.ty(v))
    }

    pub(super) fn size_of(&self, scalar: Scalar) -> Result<Size, Lack> {
        size_of(scalar).ok_or_else(|| {
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
            &InstKind::Binary { op, a, b } => {
                if op.reads() == Reads::Float {
                    let chosen = match op {
                        BinOp::FAdd => FADD,
                        BinOp::FSub => FSUB,
                        BinOp::FMul => FMUL,
                        _ => FDIV,
                    };
                    return Ok(snippet(chosen, &[a, b], self.precision(f.ty(a))?, None));
                }
                let chosen = match op {
                    BinOp::Add => ADD,
                    BinOp::Sub => SUB,
                    BinOp::And => AND,
                    BinOp::Or => OR,
                    BinOp::Xor => XOR,
                    BinOp::Mul => MUL,
                    BinOp::UDiv => UDIV,
                    BinOp::URem => UREM,
                    BinOp::SDiv => SDIV,
                    BinOp::SRem => SREM,
                    BinOp::Shl => SHL,
                    BinOp::LShr => SHR,
                    BinOp::AShr => SAR,
                    BinOp::FAdd | BinOp::FSub | BinOp::FMul | BinOp::FDiv => {
                        unreachable!("floats are read as floats")
                    }
                };
                snippet(chosen, &[a, b], self.size(a)?, None)
            }
            &InstKind::Compare { cond, a, b } => match condition(cond) {
                Some(cc) => snippet(SET, &[a, b], self.size(a)?, Some(cc)),
                None => self.float_compare(cond, a, b)?,
            },
            &InstKind::Cast { op, value, to } => self.cast(op, value, to)?,
            &InstKind::Select { cond, then, otherwise } => {
                snippet(SELECT, &[cond, then, otherwise], self.size(then)?, None)
            }
            InstKind::Call { callee, args } => {
                for &a in args {
                    self.size(a)?;
                }
                Selection::Call { callee, args }
            }
            &InstKind::Load { ty, addr, .. } => snippet(LOAD, &[addr], self.size_of(ty)?, None),
            &InstKind::Store { value, addr, access } => {
                let chosen = if access == Access::Atomic(Order::SeqCst) { STORE_SEQ_CST } else { STORE };
                snippet(chosen, &[value, addr], self.size(value)?, None)
            }
            &InstKind::Rmw { op, addr, value, .. } => {
                let size = self.size(value)?;
                match op {
                    RmwOp::Swap => snippet(SWAP, &[addr, value], size, None),
                    RmwOp::Add => snippet(FETCH_ADD, &[addr, value], size, None),
                    RmwOp::Sub => snippet(FETCH_SUB, &[addr, value], size, None),
                    RmwOp::And => Selection::Fetch { op: Op::And, addr, value, size },
                    RmwOp::Or => Selection::Fetch { op: Op::Or, addr, value, size },
                    RmwOp::Xor => Selection::Fetch { op: Op::Xor, addr, value, size },
                }
            }
            &InstKind::CompareExchange { addr, expected, new, .. } => {
                snippet(COMPARE_EXCHANGE, &[addr, expected, new], self.size(new)?, None)
            }
            &InstKind::Offset { base, offset } => snippet(ADD, &[base, offset], Size::B64, None),
            &InstKind::Copy { to, from, bytes } => Selection::Copy { to, from, bytes },
            InstKind::Asm(asm) => {
                asm.check::<X86>()?;
                Selection::Asm(asm)
            }
            InstKind::Const { .. } | InstKind::SlotAddr(_) | InstKind::Addr(_) => {
                unreachable!("recomputed where used")
            }
        })
    }

    /// A float comparison, ordered as LIR's are: false where either operand is a NaN.
    /// Unordered operands set the zero, parity and carry flags: `a < b` is tested as `b > a`,
    /// above, which fails where unordered, and (in)equality also needs the parity clear.
    fn float_compare(&self, cond: Cond, a: Value, b: Value) -> Result<Selection<'a>, Lack> {
        let precision = self.precision(self.0.f.ty(a))?;
        let (snippet, ins, cc) = match cond {
            Cond::FLt => (FCOMPARE, [b, a], Cc::A),
            Cond::FLe => (FCOMPARE, [b, a], Cc::Ae),
            Cond::FGt => (FCOMPARE, [a, b], Cc::A),
            Cond::FGe => (FCOMPARE, [a, b], Cc::Ae),
            Cond::FEq => (FEQUAL, [a, b], Cc::Z),
            _ => (FEQUAL, [a, b], Cc::Nz),
        };
        Ok(Selection::Snippet { snippet, ins: ins.to_vec(), size: precision, cond: Some(cc) })
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
                let (precision, size) = (self.precision(to)?, self.size(value)?);
                match (op, precision) {
                    (CastOp::UToF, _) if size == Size::B64 => Selection::UnsignedToFloat { value, precision },
                    (CastOp::SToF, Size::B32) => snippet(SIGNED_TO_SINGLE, size),
                    (CastOp::SToF, _) => snippet(SIGNED_TO_DOUBLE, size),
                    (_, Size::B32) => snippet(UNSIGNED_TO_SINGLE, size),
                    _ => snippet(UNSIGNED_TO_DOUBLE, size),
                }
            }
            CastOp::FToS | CastOp::FToU => {
                let (precision, size) = (self.precision(from)?, self.size_of(to)?);
                if op == CastOp::FToU && size == Size::B64 {
                    Selection::FloatToUnsigned { value, precision }
                } else {
                    snippet(if precision == Size::B32 { SINGLE_TO_INT } else { DOUBLE_TO_INT }, Size::B64)
                }
            }
        })
    }
}
