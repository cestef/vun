//! The types instructions and terminators accept and give.

use crate::entity::{Block, Inst, Value};
use crate::inst::{Address, BinOp, CastOp, Cond, InstKind, Terminator};
use crate::ty::Scalar;
use crate::unit::Storage;

use super::{Checker, Entity, Malformed, Reason};

impl Checker<'_> {
    pub(super) fn expect(&self, at: Entity, value: Value, expected: Scalar) -> Result<(), Malformed> {
        let found = self.func.ty(value);
        if found == expected { Ok(()) } else { Self::fail(at, Reason::Type { expected, found }) }
    }

    /// The address-sized integer.
    pub(super) fn word(&self) -> Scalar {
        Scalar::int(self.unit.pointer_bits).unwrap_or(Scalar::I64)
    }

    pub(super) fn inst(&self, inst: Inst) -> Result<(), Malformed> {
        let at = Entity::Inst(self.id, inst);
        let data = &self.func.insts[inst];
        let result = data.result.map(|v| self.func.ty(v));
        let gives = |ty: Option<Scalar>| {
            if result == ty {
                Ok(())
            } else {
                Self::fail(
                    at,
                    Reason::Type { expected: ty.unwrap_or(Scalar::I8), found: result.unwrap_or(Scalar::I8) },
                )
            }
        };
        match &data.kind {
            InstKind::Const { ty, bits } => {
                let width = ty.bits(self.unit.pointer_bits);
                if width < u128::BITS && *bits >> width != 0 {
                    return Self::fail(at, Reason::Constant);
                }
                gives(Some(*ty))
            }
            InstKind::Binary { op, a, b } => {
                let ty = self.func.ty(*a);
                self.expect(at, *b, ty)?;
                let float = matches!(op, BinOp::FAdd | BinOp::FSub | BinOp::FMul | BinOp::FDiv);
                if (float && !ty.is_float()) || (!float && !ty.is_int()) {
                    return Self::fail(at, Reason::Operation(ty));
                }
                gives(Some(ty))
            }
            InstKind::Compare { cond, a, b } => {
                let ty = self.func.ty(*a);
                self.expect(at, *b, ty)?;
                let float =
                    matches!(cond, Cond::FEq | Cond::FNe | Cond::FLt | Cond::FLe | Cond::FGt | Cond::FGe);
                if (float && !ty.is_float()) || (!float && ty.is_float()) {
                    return Self::fail(at, Reason::Operation(ty));
                }
                gives(Some(Scalar::I8))
            }
            InstKind::Cast { op, value, to } => {
                self.cast(at, *op, self.func.ty(*value), *to)?;
                gives(Some(*to))
            }
            InstKind::Select { cond, then, otherwise } => {
                self.expect(at, *cond, Scalar::I8)?;
                self.expect(at, *otherwise, self.func.ty(*then))?;
                gives(Some(self.func.ty(*then)))
            }
            InstKind::Load { ty, addr, .. } => {
                self.expect(at, *addr, Scalar::Ptr)?;
                gives(Some(*ty))
            }
            InstKind::Store { addr, .. } => {
                self.expect(at, *addr, Scalar::Ptr)?;
                gives(None)
            }
            InstKind::Rmw { addr, value, .. } => {
                self.expect(at, *addr, Scalar::Ptr)?;
                gives(Some(self.func.ty(*value)))
            }
            InstKind::CompareExchange { addr, expected, new, .. } => {
                self.expect(at, *addr, Scalar::Ptr)?;
                self.expect(at, *new, self.func.ty(*expected))?;
                gives(Some(self.func.ty(*expected)))
            }
            InstKind::GrowMemory { pages } => {
                self.expect(at, *pages, Scalar::I32)?;
                gives(Some(Scalar::I32))
            }
            InstKind::Offset { base, offset } => {
                self.expect(at, *base, Scalar::Ptr)?;
                self.expect(at, *offset, self.word())?;
                gives(Some(Scalar::Ptr))
            }
            InstKind::SlotAddr(slot) => {
                if self.func.slots.get(*slot).is_none() {
                    return Self::fail(at, Reason::Placement(inst));
                }
                gives(Some(Scalar::Ptr))
            }
            InstKind::Addr(address) => {
                self.unit.address(at, *address)?;
                self.unit.stored(at, *address, Storage::Program)?;
                gives(Some(Scalar::Ptr))
            }
            InstKind::ThreadAddr(data) => {
                self.unit.address(at, Address::Data(*data))?;
                self.unit.stored(at, Address::Data(*data), Storage::Thread)?;
                gives(Some(Scalar::Ptr))
            }
            InstKind::Copy { to, from, .. } => {
                self.expect(at, *to, Scalar::Ptr)?;
                self.expect(at, *from, Scalar::Ptr)?;
                gives(None)
            }
            InstKind::Call { .. } => self.call(inst),
            InstKind::Asm(asm, check) => {
                self.asm(inst, asm, check.as_ref())?;
                gives(None)
            }
        }
    }

    /// Whether `op` converts `from` to `to`.
    fn cast(&self, at: Entity, op: CastOp, from: Scalar, to: Scalar) -> Result<(), Malformed> {
        let bits = |s: Scalar| s.bits(self.unit.pointer_bits);
        let valid = match op {
            CastOp::ZExt | CastOp::SExt => from.is_int() && to.is_int() && bits(from) < bits(to),
            CastOp::Trunc => from.is_int() && to.is_int() && bits(from) > bits(to),
            CastOp::FConvert => from.is_float() && to.is_float() && from != to,
            CastOp::SToF | CastOp::UToF => from.is_int() && to.is_float(),
            CastOp::FToS | CastOp::FToU => from.is_float() && to.is_int(),
            CastOp::PtrToInt => from == Scalar::Ptr && to == self.word(),
            CastOp::IntToPtr => from == self.word() && to == Scalar::Ptr,
            CastOp::Bitcast => from != to && bits(from) == bits(to),
        };
        if valid { Ok(()) } else { Self::fail(at, Reason::Operation(from)) }
    }

    pub(super) fn term(&self, block: Block) -> Result<(), Malformed> {
        let at = Entity::Block(self.id, block);
        match &self.func.blocks[block].term {
            Terminator::Branch { cond, .. } => self.expect(at, *cond, Scalar::I8),
            Terminator::Switch { value, .. } if !self.func.ty(*value).is_int() => {
                Self::fail(at, Reason::Operation(self.func.ty(*value)))
            }
            Terminator::Return(value) => {
                let found = value.map(|v| self.func.ty(v));
                if found == self.func.signature.returns() { Ok(()) } else { Self::fail(at, Reason::Return) }
            }
            Terminator::Switch { .. } | Terminator::Jump(_) | Terminator::Unreachable => Ok(()),
        }
    }
}
