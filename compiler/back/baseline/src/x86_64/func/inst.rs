//! Instruction selection: one method per kind of instruction, over the operation facts of LIR
//! (`Reads`): arithmetic, comparisons, conversions and selects here, memory and calls apart.

use vun_backend::{Lack, Missing};
use vun_lir::func::Def;
use vun_lir::{BinOp, Block, CastOp, Cond, InstKind, Reads, Scalar, Terminator, Value};

use super::super::encode::{Alu, Shift, Unary};
use super::super::{Cc, Reg, Rm, Size};
use super::FnCompiler;

impl FnCompiler<'_> {
    /// The size of an integer or address `v`, or what the backend lacks for it.
    pub(super) fn size(&self, v: Value) -> Result<Size, Lack> {
        self.size_of(self.f.ty(v))
    }

    pub(super) fn size_of(&self, scalar: Scalar) -> Result<Size, Lack> {
        Size::of(scalar).ok_or_else(|| {
            self.lack(match scalar {
                Scalar::I128 => Missing::Int128,
                Scalar::F16 => Missing::Half,
                _ => Missing::Float,
            })
        })
    }

    pub(super) fn select(&mut self, kind: &InstKind, result: Option<Value>) -> Result<(), Lack> {
        let out = || result.expect("the instruction defines a value");
        match kind {
            InstKind::Binary { op, a, b } => self.binary(*op, *a, *b, out()),
            InstKind::Compare { cond, a, b } => {
                let cc = self.compare(*cond, *a, *b)?;
                let d = self.def(out());
                self.asm.setcc(cc, d);
                Ok(())
            }
            InstKind::Cast { op, value, to } => self.cast(*op, *value, *to, out()),
            InstKind::Select { cond, then, otherwise } => {
                self.size(out())?;
                let (c, t, o) = (self.get(*cond), self.get(*then), self.get(*otherwise));
                let d = self.def(out());
                self.asm.mov_rr(d, o);
                self.asm.test(Size::B8, c, c);
                self.asm.cmov(Cc::Ne, d, t);
                Ok(())
            }
            InstKind::Call { callee, args } => self.call(callee, args, result),
            _ => self.memory(kind, result),
        }
    }

    /// `cmp a, b`: the flags, and the condition code that holds where `cond` does.
    pub(super) fn compare(&mut self, cond: Cond, a: Value, b: Value) -> Result<Cc, Lack> {
        let cc = Cc::of(cond).ok_or_else(|| self.lack(Missing::Float))?;
        let size = self.size(a)?;
        let x = self.get(a);
        if let Some(imm) = self.constant(b) {
            self.asm.alu_imm(Alu::Cmp, size, Rm::Reg(x), imm);
        } else {
            let y = self.get(b);
            self.asm.alu(Alu::Cmp, size, x, y);
        }
        Ok(cc)
    }

    /// The comparison `block` branches on, when nothing else uses its result and it is made in
    /// `block`: the branch compares and jumps on the flags, the condition never materialized.
    pub(super) fn fused(&self, block: Block) -> Option<(Value, Cond, Value, Value)> {
        let Terminator::Branch { cond: v, .. } = self.f.blocks[block].term else { return None };
        let Def::Inst(inst) = self.f.values[v].def else { return None };
        let InstKind::Compare { cond, a, b } = self.f.insts[inst].kind else { return None };
        let made_here = self.f.blocks[block].insts.contains(&inst);
        (made_here && self.left[v] == 1 && Cc::of(cond).is_some()).then_some((v, cond, a, b))
    }

    fn binary(&mut self, op: BinOp, a: Value, b: Value, out: Value) -> Result<(), Lack> {
        let size = self.size(out)?;
        let alu = match op {
            BinOp::Add => Some(Alu::Add),
            BinOp::Sub => Some(Alu::Sub),
            BinOp::And => Some(Alu::And),
            BinOp::Or => Some(Alu::Or),
            BinOp::Xor => Some(Alu::Xor),
            _ => None,
        };
        if let Some(alu) = alu {
            let left = self.get(a);
            let imm = self.constant(b);
            let right = if imm.is_none() { Some(self.get(b)) } else { None };
            let dst = self.def(out);
            self.asm.mov_rr(dst, left);
            match (imm, right) {
                (Some(imm), _) => self.asm.alu_imm(alu, size.wide(), Rm::Reg(dst), imm),
                (None, Some(right)) => self.asm.alu(alu, size.wide(), dst, right),
                (None, None) => unreachable!("an operand is a constant or a register"),
            }
            return Ok(());
        }
        match op {
            BinOp::Mul => {
                let (left, right) = (self.get(a), self.get(b));
                let dst = self.def(out);
                self.asm.mov_rr(dst, left);
                self.asm.imul(size.wide(), dst, right);
                Ok(())
            }
            BinOp::SDiv | BinOp::UDiv | BinOp::SRem | BinOp::URem => {
                self.divide(op, a, b, out, size);
                Ok(())
            }
            BinOp::Shl | BinOp::LShr | BinOp::AShr => {
                self.shift(op, a, b, out, size);
                Ok(())
            }
            _ => Err(self.lack(Missing::Float)),
        }
    }

    /// Writes `v`, of `size`, into `dst` extended to the width arithmetic runs at, as `reads`
    /// reads it: bits above a value's own are undefined in a register.
    fn widened(&mut self, v: Value, dst: Reg, size: Size, reads: Reads) {
        self.get_into(v, dst);
        if size < Size::B32 {
            match reads {
                Reads::Signed => self.asm.sign_extend(size, dst, dst),
                _ => self.asm.zero_extend(size, dst, dst),
            }
        }
    }

    /// Division and remainder through `rax` and `rdx`.
    fn divide(&mut self, op: BinOp, a: Value, b: Value, out: Value, size: Size) {
        let reads = op.reads();
        self.widened(a, Reg::TEMP, size, reads);
        self.widened(b, Reg::SCRATCH, size, reads);
        self.take(Reg::Rax);
        self.take(Reg::Rdx);
        let wide = size.wide();
        self.asm.mov_rr(Reg::Rax, Reg::TEMP);
        let unary = if reads == Reads::Signed {
            self.asm.sign_extend_rax(wide);
            Unary::Idiv
        } else {
            self.asm.alu(Alu::Xor, Size::B32, Reg::Rdx, Reg::Rdx);
            Unary::Div
        };
        self.asm.unary(unary, wide, Rm::Reg(Reg::SCRATCH));
        let result = if matches!(op, BinOp::SDiv | BinOp::UDiv) { Reg::Rax } else { Reg::Rdx };
        self.bind(out, result, false);
    }

    /// Shifts by `cl`, of a value extended as the shift reads it.
    fn shift(&mut self, op: BinOp, a: Value, b: Value, out: Value, size: Size) {
        self.widened(a, Reg::TEMP, size, op.reads());
        self.get_into(b, Reg::SCRATCH);
        self.take(Reg::Rcx);
        self.asm.mov_rr(Reg::Rcx, Reg::SCRATCH);
        let shift = match op {
            BinOp::Shl => Shift::Shl,
            BinOp::LShr => Shift::Shr,
            _ => Shift::Sar,
        };
        self.asm.shift(shift, size.wide(), Reg::TEMP);
        let d = self.def(out);
        self.asm.mov_rr(d, Reg::TEMP);
    }

    fn cast(&mut self, op: CastOp, value: Value, to: Scalar, out: Value) -> Result<(), Lack> {
        let from = self.size(value)?;
        self.size_of(to)?;
        let x = self.get(value);
        let d = self.def(out);
        match op {
            CastOp::ZExt => self.asm.zero_extend(from, d, x),
            CastOp::SExt => self.asm.sign_extend(from, d, x),
            CastOp::Trunc | CastOp::PtrToInt | CastOp::IntToPtr | CastOp::Bitcast => self.asm.mov_rr(d, x),
            CastOp::FConvert | CastOp::SToF | CastOp::UToF | CastOp::FToS | CastOp::FToU => {
                return Err(self.lack(Missing::Float));
            }
        }
        Ok(())
    }
}
