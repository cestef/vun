//! The instruction compilers of arm64: most operations are snippets; calls, memory copies, loops
//! of atomic operations, the frame and the terminators are written here.

mod atomic;
mod call;
mod carry;
mod entry;
mod float;
mod memory;
mod term;

use vun_asm_arm64::{Cc, Size};
use vun_backend::{Lack, Missing};
use vun_codegen::snippet::{Args, Snippet};
use vun_codegen::{Cx, Isel};
use vun_lir::{BinOp, Block, CastOp, Cond, Function, Inst, InstKind, Reads, Scalar, Value};

use vun_abi::Convention;

use super::abi::Passing;
use super::snippets::{
    ADD, AND, CMP, CMP_SIGNED, CMP_UNSIGNED, KEEP, MUL, OR, SAR, SAR_NARROW, SDIV, SDIV_NARROW, SELECT, SET,
    SET_SIGNED, SET_UNSIGNED, SEXT, SHL, SHR, SHR_NARROW, SREM, SREM_NARROW, SUB, UDIV, UDIV_NARROW, UREM,
    UREM_NARROW, XOR, ZEXT,
};
use super::{Arm, condition};
use crate::width::size_of;

/// What the instruction compilers keep for one function.
pub struct State {
    /// The variant of the call standard the target follows.
    convention: &'static Convention<vun_asm_arm64::Reg, vun_asm_arm64::V>,
    /// Where the function's own signature passes each value.
    passing: Passing,
    /// Where the prologue's frame size is written once known.
    frame_at: u64,
    /// Where a result returned in registers is gathered.
    gathered: Option<i32>,
}

/// The instruction compilers over the function compiled.
struct Sel<'c, 'a>(&'c mut Cx<'a, Arm>);

impl<'a> std::ops::Deref for Sel<'_, 'a> {
    type Target = Cx<'a, Arm>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::ops::DerefMut for Sel<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl Isel for Arm {
    type Convention = &'static Convention<vun_asm_arm64::Reg, vun_asm_arm64::V>;

    fn state(f: &Function, convention: Self::Convention) -> Result<State, Lack> {
        let passing = Passing::of(&f.signature, convention).map_err(|u| Missing::from(u).at(f.origin))?;
        Ok(State { convention, passing, frame_at: 0, gathered: None })
    }

    fn fuses(cond: Cond) -> bool {
        condition(cond).is_some()
    }

    fn prologue(cx: &mut Cx<'_, Self>) -> Result<(), Lack> {
        Sel(cx).prologue();
        Ok(())
    }

    fn entry(cx: &mut Cx<'_, Self>) -> Result<(), Lack> {
        Sel(cx).entry();
        Ok(())
    }

    fn inst(cx: &mut Cx<'_, Self>, inst: Inst) -> Result<(), Lack> {
        Sel(cx).inst(inst)
    }

    fn terminate(cx: &mut Cx<'_, Self>, block: Block) -> Result<(), Lack> {
        Sel(cx).terminate(block)
    }

    fn finish(cx: &mut Cx<'_, Self>, frame: u64) {
        Sel(cx).finish(frame);
    }
}

/// Whether an operation of `size` reads narrow operands, which it extends first.
fn narrow(size: Size) -> bool {
    size < Size::B32
}

impl Sel<'_, '_> {
    fn inst(&mut self, inst: Inst) -> Result<(), Lack> {
        let f = self.f;
        let data = &f.insts[inst];
        let out = data.result;
        match &data.kind {
            &InstKind::Binary { op, a, b } => self.binary(op, a, b, out),
            &InstKind::Compare { cond, a, b } => self.compare(cond, a, b, out),
            &InstKind::Cast { op, value, to } => self.cast(op, value, to, out),
            &InstKind::Select { cond, then, otherwise } => {
                self.size(then)?;
                self.apply(SELECT, &[cond, then, otherwise], out, Size::B64, None);
                Ok(())
            }
            InstKind::Call { callee, args } => self.call(callee, args, out),
            kind => self.memory(kind, out),
        }
    }

    /// The size of an integer, address or float value, or what the backend lacks for it.
    fn size(&self, v: Value) -> Result<Size, Lack> {
        self.size_of(self.f.ty(v))
    }

    fn size_of(&self, scalar: Scalar) -> Result<Size, Lack> {
        size_of(scalar).ok_or_else(|| {
            self.lack(match scalar {
                Scalar::I128 => Missing::Int128,
                _ => Missing::Half,
            })
        })
    }

    /// Emits `snippet` over `ins`, defining `out`.
    fn apply(
        &mut self,
        snippet: Snippet<Arm>,
        ins: &[Value],
        out: Option<Value>,
        size: Size,
        cond: Option<Cc>,
    ) {
        self.snippet(snippet, &Args { ins, out, size, cond });
    }

    fn binary(&mut self, op: BinOp, a: Value, b: Value, out: Option<Value>) -> Result<(), Lack> {
        if op.reads() == Reads::Float {
            return self.float_binary(op, a, b, out);
        }
        let size = self.size(a)?;
        let snippet = match (op, narrow(size)) {
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
        self.apply(snippet, &[a, b], out, size, None);
        Ok(())
    }

    fn compare(&mut self, cond: Cond, a: Value, b: Value, out: Option<Value>) -> Result<(), Lack> {
        if cond.reads() == Reads::Float {
            return self.float_compare(cond, a, b, out);
        }
        let size = self.size(a)?;
        let snippet = match (narrow(size), cond.reads()) {
            (false, _) => SET,
            (true, Reads::Signed) => SET_SIGNED,
            (true, _) => SET_UNSIGNED,
        };
        self.apply(snippet, &[a, b], out, size, condition(cond));
        Ok(())
    }

    /// Sets the flags `cond` holds on for `a` and `b`, as the branch fusing the comparison reads
    /// them.
    fn flags(&mut self, cond: Cond, a: Value, b: Value) -> Result<(), Lack> {
        let size = self.size(a)?;
        let snippet = match (cond.reads(), narrow(size)) {
            (Reads::Float, _) => super::snippets::FCMP,
            (_, false) => CMP,
            (Reads::Signed, true) => CMP_SIGNED,
            (_, true) => CMP_UNSIGNED,
        };
        self.apply(snippet, &[a, b], None, size, None);
        Ok(())
    }

    fn cast(&mut self, op: CastOp, value: Value, to: Scalar, out: Option<Value>) -> Result<(), Lack> {
        let snippet = match op {
            CastOp::ZExt => ZEXT,
            CastOp::SExt => SEXT,
            CastOp::Trunc | CastOp::PtrToInt | CastOp::IntToPtr | CastOp::Bitcast => KEEP,
            CastOp::FConvert | CastOp::SToF | CastOp::UToF | CastOp::FToS | CastOp::FToU => {
                return self.float_cast(op, value, to, out);
            }
        };
        self.size_of(to)?;
        let size = self.size(value)?;
        self.apply(snippet, &[value], out, size, None);
        Ok(())
    }
}
