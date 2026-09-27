//! The instruction compilers of x86-64: most operations are snippets; calls, memory copies,
//! loops of atomic operations, unsigned 64-bit float conversions, the frame and the
//! terminators are written here.

mod call;
mod carry;
mod entry;
mod float;
mod memory;
mod term;

use vun_backend::{Lack, Missing};
use vun_codegen::snippet::{Args, Snippet};
use vun_codegen::{Cx, Isel};
use vun_lir::{BinOp, Block, CastOp, Cond, Function, Inst, InstKind, Reads, Scalar, Value};

use super::abi::Passing;
use super::snippets::{
    ADD, AND, KEEP, MUL, OR, SAR, SDIV, SELECT, SET, SEXT, SHL, SHR, SREM, SUB, UDIV, UREM, XOR, ZEXT,
};
use super::{X86, condition};
use crate::width::size_of;
use vun_asm_x86_64::{Cc, Size};

/// What the instruction compilers keep for one function.
pub struct State {
    convention: &'static vun_abi::Convention<vun_asm_x86_64::Reg, vun_asm_x86_64::Xmm>,
    /// Where the function's own signature passes each value.
    passing: Passing,
    /// Where the prologue's `sub rsp, frame` is, and its immediate, written once the size is known.
    frame_at: u64,
    frame_imm: u64,
    /// The home of the address a result is written through, which is given back.
    hidden: Option<i32>,
    /// Where a result returned in registers is gathered.
    gathered: Option<i32>,
}

/// The instruction compilers over the function compiled.
struct Sel<'c, 'a>(&'c mut Cx<'a, X86>);

impl<'a> std::ops::Deref for Sel<'_, 'a> {
    type Target = Cx<'a, X86>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::ops::DerefMut for Sel<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl Isel for X86 {
    type Convention = &'static vun_abi::Convention<vun_asm_x86_64::Reg, vun_asm_x86_64::Xmm>;

    fn state(f: &Function, convention: Self::Convention) -> Result<State, Lack> {
        let passing = Passing::of(&f.signature, convention).map_err(|u| Missing::from(u).at(f.origin))?;
        Ok(State { convention, passing, frame_at: 0, frame_imm: 0, hidden: None, gathered: None })
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
        Sel(cx).frame(frame);
    }
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
                let size = self.size(then)?;
                self.apply(SELECT, &[cond, then, otherwise], out, size, None);
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
        snippet: Snippet<X86>,
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
        let snippet = match op {
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
        let size = self.size(a)?;
        self.apply(snippet, &[a, b], out, size, None);
        Ok(())
    }

    fn compare(&mut self, cond: Cond, a: Value, b: Value, out: Option<Value>) -> Result<(), Lack> {
        let Some(cc) = condition(cond) else { return self.float_compare(cond, a, b, out) };
        let size = self.size(a)?;
        self.apply(SET, &[a, b], out, size, Some(cc));
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
