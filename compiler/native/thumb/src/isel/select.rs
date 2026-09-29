//! How each instruction is compiled, decided once: the snippet it is, over which operands, at
//! which size, or a sequence written here. What it needs of the plan and what it emits both
//! read the same decision.

use vun_asm_thumb::Size;
use vun_backend::{Lack, Missing};
use vun_codegen::Cx;
use vun_codegen::snippet::Snippet;
use vun_lir::{BinOp, CastOp, Cond, Inst, InstKind, Reads, Scalar, Value};

use crate::snippets::{
    ADD, AND, CMP, CMP_SIGNED, CMP_UNSIGNED, FADD, FCMP, FDIV, FMUL, FNE, FSET, FSUB, KEEP, LOAD,
    LOAD_ACQUIRE, MUL, NARROW_SIGNED_TO_SINGLE, NARROW_UNSIGNED_TO_SINGLE, OR, SAR, SAR_NARROW, SDIV,
    SDIV_NARROW, SELECT, SET, SET_SIGNED, SET_UNSIGNED, SEXT, SHL, SHR, SHR_NARROW, SIGNED_TO_SINGLE,
    SINGLE_TO_SIGNED, SINGLE_TO_UNSIGNED, SREM, SREM_NARROW, STORE, STORE_RELEASE, SUB, UDIV, UDIV_NARROW,
    UNSIGNED_TO_SINGLE, UREM, UREM_NARROW, XOR, ZEXT,
};
use crate::{Flags, Thumb2};
use vun_native::inline::Inlined;
use vun_native::width::size_of;

/// An instruction as it is compiled.
pub(super) type Selection<'f> = vun_native::select::Selection<'f, Thumb2>;

/// The function compiled, read without emitting.
pub(super) struct Look<'c, 'a>(pub(super) &'c Cx<'a, Thumb2>);

impl<'a> Look<'_, 'a> {
    /// The size of an integer, address or float value, or what the backend lacks for it.
    pub(super) fn size(&self, v: Value) -> Result<Size, Lack> {
        self.size_of(self.0.f.ty(v))
    }

    /// The size of `scalar` in a general register: a word at most.
    pub(super) fn size_of(&self, scalar: Scalar) -> Result<Size, Lack> {
        match size_of(scalar, self.0.unit.pointer_bits) {
            Some(Size::B64) => Err(self.0.lack(Missing::Wider(8))),
            Some(size) => Ok(size),
            None => Err(self.0.lack(match scalar {
                Scalar::I128 => Missing::Int128,
                _ => Missing::Half,
            })),
        }
    }

    /// The size of the bits of the float scalar `scalar`, which is its precision: single; the
    /// instructions computing it are the processor's where it has a floating-point unit.
    fn precision(&self, scalar: Scalar) -> Result<Size, Lack> {
        match scalar {
            Scalar::F32 => Ok(Size::B32),
            Scalar::F64 => Err(self.0.lack(Missing::Wider(8))),
            _ => Err(self.0.lack(Missing::Half)),
        }
    }

    /// An operation reading or giving a double, which lives in its home (`Arch::bank`), as the
    /// isel writes it; `None` for another, and for a call, which passes doubles as calls do.
    fn homed(&self, inst: Inst) -> Result<Option<Selection<'a>>, Lack> {
        let cx = self.0;
        let data = &cx.f.insts[inst];
        let touched = data.result.is_some_and(|v| cx.homed(v)) || data.kind.operands().any(|v| cx.homed(v));
        let word = |v: Value| self.size(v).is_ok();
        let computed = match &data.kind {
            _ if !touched => return Ok(None),
            InstKind::Call { .. } => return Ok(None),
            InstKind::Const { .. } | InstKind::Select { .. } => true,
            InstKind::Binary { op, .. } => {
                matches!(op, BinOp::FAdd | BinOp::FSub | BinOp::FMul | BinOp::FDiv)
            }
            InstKind::Compare { cond, .. } => cond.holds().is_some(),
            // Between the precisions, and with integers of a word at most.
            &InstKind::Cast { op, value, to } => match op {
                CastOp::FConvert => true,
                CastOp::SToF | CastOp::UToF => word(value),
                CastOp::FToS | CastOp::FToU => self.size_of(to).is_ok(),
                _ => false,
            },
            InstKind::Load { access, .. } | InstKind::Store { access, .. } => {
                !matches!(access, vun_lir::Access::Atomic(_))
            }
            _ => false,
        };
        if !computed {
            return Err(cx.lack(Missing::Wider(8)));
        }
        Ok(Some(Selection::Homed { ins: data.kind.operands().collect() }))
    }

    /// What the backend lacks to pass `args`: a word at most each, but a double, which travels
    /// from its home.
    fn passed(&self, args: &[Value]) -> Result<(), Lack> {
        for &a in args.iter().filter(|&&a| !self.0.homed(a)) {
            self.size(a)?;
        }
        Ok(())
    }

    /// How `inst` is compiled.
    #[expect(clippy::too_many_lines, reason = "one arm per kind of instruction")]
    pub(super) fn selection(&self, inst: Inst) -> Result<Selection<'a>, Lack> {
        if let Some(homed) = self.homed(inst)? {
            return Ok(homed);
        }
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
                match cond.holds() {
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
                snippet(chosen, &[a, b], size, cond.holds())
            }
            &InstKind::Cast { op, value, to } => self.cast(op, value, to)?,
            &InstKind::Select { cond, then, otherwise } => {
                self.size(then)?;
                snippet(SELECT, &[cond, then, otherwise], Size::B32, None)
            }
            InstKind::Call { callee, args } => {
                self.passed(args)?;
                Selection::Call { callee, args }
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
                self.word(value)?;
                Selection::Rmw { op, addr, value, size: Size::B32 }
            }
            &InstKind::CompareExchange { addr, expected, new, .. } => {
                self.word(new)?;
                Selection::CompareExchange { addr, expected, new, size: Size::B32 }
            }
            &InstKind::Offset { base, offset } => snippet(ADD, &[base, offset], Size::B32, None),
            &InstKind::Copy { to, from, bytes } => Selection::Copy { to, from, bytes },
            InstKind::Asm(asm, check) => {
                asm.check::<Thumb2>()?;
                Selection::Asm(asm, check.as_ref())
            }
            &InstKind::ThreadAddr(data) => Selection::ThreadAddr { data },
            InstKind::Const { .. } | InstKind::SlotAddr(_) | InstKind::Addr(_) => {
                unreachable!("recomputed where used")
            }
        })
    }

    /// Whether `v` is a word, which exclusive accesses alone take.
    fn word(&self, v: Value) -> Result<(), Lack> {
        match self.size(v)? {
            Size::B32 => Ok(()),
            _ => Err(self.0.lack(Missing::Atomic)),
        }
    }

    /// The flags a branch fusing the comparison `cond` of `a` and `b` reads, as a snippet.
    pub(super) fn flags(&self, cond: Cond, a: Value) -> Result<(Snippet<Thumb2>, Size), Lack> {
        let reads = cond.reads();
        let size = if reads == Reads::Float { self.precision(self.0.f.ty(a))? } else { self.size(a)? };
        let snippet = match (reads, size.narrow()) {
            (Reads::Float, _) => FCMP,
            (_, false) => CMP,
            (Reads::Signed, true) => CMP_SIGNED,
            (_, true) => CMP_UNSIGNED,
        };
        Ok((snippet, size))
    }

    /// Conversions between integers, and between floats and integers; a narrower integer result
    /// is the low bits of the 32-bit one.
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
            // One precision only: a conversion between two is to or from a double.
            CastOp::FConvert => {
                self.precision(from)?;
                self.precision(to)?;
                snippet(KEEP, Size::B32)
            }
            CastOp::SToF | CastOp::UToF => {
                let (precision, size) = (self.precision(to)?, self.size(value)?);
                let chosen = match (op == CastOp::SToF, size.narrow()) {
                    (true, false) => SIGNED_TO_SINGLE,
                    (false, false) => UNSIGNED_TO_SINGLE,
                    (true, true) => NARROW_SIGNED_TO_SINGLE,
                    (false, true) => NARROW_UNSIGNED_TO_SINGLE,
                };
                snippet(chosen, if size.narrow() { size } else { precision })
            }
            CastOp::FToS | CastOp::FToU => {
                let precision = self.precision(from)?;
                self.size_of(to)?;
                snippet(if op == CastOp::FToS { SINGLE_TO_SIGNED } else { SINGLE_TO_UNSIGNED }, precision)
            }
        })
    }
}
