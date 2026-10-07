//! Instructions: each LIR instruction as the Cranelift ones computing it.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::immediates::{Ieee32, Ieee64};
use cranelift_codegen::ir::{AtomicRmwOp, InstBuilder, Value as Val};
use vun_backend::{Lack, Missing};
use vun_lir::{Access, BinOp, CallKind, Cond, Inst, InstKind, RmwOp, Scalar};
use vun_origin::OriginId;

use super::Lower;

impl Lower<'_, '_> {
    pub(super) fn inst(&mut self, id: Inst) -> Result<(), Lack> {
        let (f, data) = (self.f, &self.f.insts[id]);
        self.locate(data.origin);
        vun_trace::tracing::trace!(inst = vun_index::Idx::index(id), kind = ?data.kind, "lowering");
        let at = data.origin;
        let result = match &data.kind {
            InstKind::Const { ty, bits } => Some(self.constant(*ty, *bits, at)?),
            InstKind::Binary { op, a, b } => Some(self.binary(*op, self.get(*a), self.get(*b))),
            InstKind::Compare { cond, a, b } => Some(self.compare(*cond, self.get(*a), self.get(*b))),
            InstKind::Cast { op, value, to } => Some(self.cast(*op, self.get(*value), *to, at)?),
            InstKind::Select { cond, then, otherwise } => {
                let [cond, then, otherwise] = [*cond, *then, *otherwise].map(|v| self.get(v));
                Some(self.b.ins().select(cond, then, otherwise))
            }
            InstKind::Load { ty, addr, access } => Some(self.load(*ty, self.get(*addr), *access, at)?),
            InstKind::Store { value, addr, access } => {
                self.store(self.get(*value), self.get(*addr), *access);
                None
            }
            InstKind::Rmw { op, addr, value, .. } => {
                let ty = self.ty(f.ty(*value), at)?;
                let op = Self::rmw(*op);
                let [addr, value] = [*addr, *value].map(|v| self.get(v));
                Some(self.b.ins().atomic_rmw(ty, Self::plain(), op, addr, value))
            }
            InstKind::CompareExchange { addr, expected, new, .. } => {
                let [addr, expected, new] = [*addr, *expected, *new].map(|v| self.get(v));
                Some(self.b.ins().atomic_cas(Self::plain(), addr, expected, new))
            }
            InstKind::Offset { base, offset } => {
                let [base, offset] = [*base, *offset].map(|v| self.get(v));
                Some(self.b.ins().iadd(base, offset))
            }
            InstKind::SlotAddr(slot) => {
                let slot = self.slots[*slot].expect("slots are made first");
                Some(self.b.ins().stack_addr(self.ptr, slot, 0))
            }
            InstKind::Addr(address) => Some(self.address(*address, at)?),
            InstKind::ThreadAddr(data) => Some(self.thread_address(*data)),
            InstKind::Copy { to, from, bytes } => {
                self.copy(self.get(*to), self.get(*from), *bytes);
                None
            }
            InstKind::Call { callee, args, kind: CallKind::Tail } => {
                self.tail_call(callee, args, at)?;
                None
            }
            InstKind::Call { callee, args, .. } => {
                self.call(callee, args, f.insts[id].result.is_some(), at)?
            }
            InstKind::Asm(..) => return Err(Missing::Asm.at(at)),
        };
        if let (Some(value), Some(defined)) = (data.result, result) {
            self.values[value] = Some(defined);
        }
        Ok(())
    }

    fn constant(&mut self, ty: Scalar, bits: u128, at: OriginId) -> Result<Val, Lack> {
        let made = self.ty(ty, at)?;
        Ok(match ty {
            Scalar::F32 => {
                self.b.ins().f32const(Ieee32::with_bits(u32::try_from(bits).expect("an f32's bits")))
            }
            Scalar::F64 => {
                self.b.ins().f64const(Ieee64::with_bits(u64::try_from(bits).expect("an f64's bits")))
            }
            _ => {
                let int = if ty == Scalar::Ptr {
                    Scalar::int(self.cx.unit.pointer_bits).expect("an address")
                } else {
                    ty
                };
                let n = i64::try_from(int.signed(bits)).expect("constants fit 64 bits");
                self.b.ins().iconst(made, n)
            }
        })
    }

    fn binary(&mut self, op: BinOp, a: Val, b: Val) -> Val {
        let ins = self.b.ins();
        match op {
            BinOp::Add => ins.iadd(a, b),
            BinOp::Sub => ins.isub(a, b),
            BinOp::Mul => ins.imul(a, b),
            BinOp::SDiv => ins.sdiv(a, b),
            BinOp::UDiv => ins.udiv(a, b),
            BinOp::SRem => ins.srem(a, b),
            BinOp::URem => ins.urem(a, b),
            BinOp::And => ins.band(a, b),
            BinOp::Or => ins.bor(a, b),
            BinOp::Xor => ins.bxor(a, b),
            BinOp::Shl => ins.ishl(a, b),
            BinOp::LShr => ins.ushr(a, b),
            BinOp::AShr => ins.sshr(a, b),
            BinOp::FAdd => ins.fadd(a, b),
            BinOp::FSub => ins.fsub(a, b),
            BinOp::FMul => ins.fmul(a, b),
            BinOp::FDiv => ins.fdiv(a, b),
        }
    }

    fn compare(&mut self, cond: Cond, a: Val, b: Val) -> Val {
        match Cc::of(cond) {
            Cc::Int(cc) => self.b.ins().icmp(cc, a, b),
            Cc::Float(cc) => self.b.ins().fcmp(cc, a, b),
        }
    }

    fn rmw(op: RmwOp) -> AtomicRmwOp {
        match op {
            RmwOp::Swap => AtomicRmwOp::Xchg,
            RmwOp::Add => AtomicRmwOp::Add,
            RmwOp::Sub => AtomicRmwOp::Sub,
            RmwOp::And => AtomicRmwOp::And,
            RmwOp::Or => AtomicRmwOp::Or,
            RmwOp::Xor => AtomicRmwOp::Xor,
        }
    }

    fn load(&mut self, ty: Scalar, addr: Val, access: Access, at: OriginId) -> Result<Val, Lack> {
        let ty = self.ty(ty, at)?;
        let flags = Self::plain();
        Ok(match access {
            Access::Atomic(_) => self.b.ins().atomic_load(ty, flags, addr),
            Access::Plain | Access::Volatile => self.b.ins().load(ty, flags, addr, 0),
        })
    }

    fn store(&mut self, value: Val, addr: Val, access: Access) {
        let flags = Self::plain();
        match access {
            Access::Atomic(_) => {
                self.b.ins().atomic_store(flags, value, addr);
            }
            Access::Plain | Access::Volatile => {
                self.b.ins().store(flags, value, addr, 0);
            }
        }
    }
}

/// A comparison as Cranelift names it: of integers, or of floats, which are false of a NaN but
/// `!=`.
enum Cc {
    Int(IntCC),
    Float(FloatCC),
}

impl Cc {
    const fn of(cond: Cond) -> Self {
        match cond {
            Cond::Eq => Self::Int(IntCC::Equal),
            Cond::Ne => Self::Int(IntCC::NotEqual),
            Cond::SLt => Self::Int(IntCC::SignedLessThan),
            Cond::SLe => Self::Int(IntCC::SignedLessThanOrEqual),
            Cond::SGt => Self::Int(IntCC::SignedGreaterThan),
            Cond::SGe => Self::Int(IntCC::SignedGreaterThanOrEqual),
            Cond::ULt => Self::Int(IntCC::UnsignedLessThan),
            Cond::ULe => Self::Int(IntCC::UnsignedLessThanOrEqual),
            Cond::UGt => Self::Int(IntCC::UnsignedGreaterThan),
            Cond::UGe => Self::Int(IntCC::UnsignedGreaterThanOrEqual),
            Cond::FEq => Self::Float(FloatCC::Equal),
            Cond::FNe => Self::Float(FloatCC::NotEqual),
            Cond::FLt => Self::Float(FloatCC::LessThan),
            Cond::FLe => Self::Float(FloatCC::LessThanOrEqual),
            Cond::FGt => Self::Float(FloatCC::GreaterThan),
            Cond::FGe => Self::Float(FloatCC::GreaterThanOrEqual),
        }
    }
}
