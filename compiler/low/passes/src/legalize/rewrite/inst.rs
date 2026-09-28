//! Instructions: each on a wide integer rewritten into a sequence over limbs, each on a promoted
//! `f16` into one in `f32`, any other copied over the new values.

use super::super::{Unlegal, Unlegalized};
use super::{Parts, Rewrite};
use vun_lir::{Access, BinOp, CastOp, Inst, InstKind, Scalar, Value};
use vun_origin::OriginId;

impl Rewrite<'_> {
    pub(super) fn inst(&mut self, inst: Inst) -> Result<(), Unlegal> {
        let data = &self.old.insts[inst];
        let (at, result) = (data.origin, data.result);
        let unlegal = |why| Unlegal { at, why };
        if let Some(parts) = self.held(&data.kind).map_err(unlegal)? {
            if let Some(v) = result {
                self.parts.insert(v, parts);
            }
            return Ok(());
        }
        if let Some(parts) = self.half(&data.kind, at).map_err(unlegal)? {
            if let Some(v) = result {
                self.parts.insert(v, parts);
            }
            return Ok(());
        }
        let parts = match &data.kind {
            InstKind::Const { ty, bits } if self.legalize.limbs(*ty).is_some() => {
                let count = self.split(*ty).len();
                Parts::Limbs(self.limbwise(at).limbs(*bits, count))
            }
            InstKind::Binary { op, a, b } if self.is_wide(*a) => {
                let (x, y) = (self.limbs(*a), self.limbs(*b));
                let mut l = self.limbwise(at);
                Parts::Limbs(match op {
                    BinOp::Add => l.add(&x, &y),
                    BinOp::Sub => l.sub(&x, &y),
                    BinOp::Mul => l.mul(&x, &y),
                    BinOp::And | BinOp::Or | BinOp::Xor => l.bitwise(*op, &x, &y),
                    BinOp::Shl | BinOp::LShr | BinOp::AShr => l.shift(*op, &x, y.low()),
                    _ => self.divide(*op, &x, &y, at),
                })
            }
            InstKind::Compare { cond, a, b } if self.is_wide(*a) => {
                let (x, y) = (self.limbs(*a), self.limbs(*b));
                Parts::One(self.limbwise(at).compared(*cond, &x, &y))
            }
            &InstKind::Cast { op, value, to } if self.is_wide(value) || self.legalize.limbs(to).is_some() => {
                self.cast(op, value, to, at).map_err(unlegal)?
            }
            InstKind::Select { cond, then, otherwise } if self.is_wide(*then) => {
                let (c, t, o) = (self.one(*cond), self.limbs(*then), self.limbs(*otherwise));
                Parts::Limbs(self.limbwise(at).select_limbs(c, &t, &o))
            }
            InstKind::Load { ty, addr, access } if self.legalize.limbs(*ty).is_some() => {
                if matches!(access, Access::Atomic(_)) {
                    return Err(unlegal(Unlegalized::Atomic));
                }
                let (addr, count) = (self.one(*addr), self.split(*ty).len());
                Parts::Limbs(self.load_limbs(addr, *access, count, at))
            }
            InstKind::Store { value, addr, access } if self.is_wide(*value) => {
                if matches!(access, Access::Atomic(_)) {
                    return Err(unlegal(Unlegalized::Atomic));
                }
                let (value, addr) = (self.limbs(*value), self.one(*addr));
                self.store_limbs(&value, addr, *access, at);
                return Ok(());
            }
            InstKind::Rmw { value, .. } | InstKind::CompareExchange { new: value, .. }
                if self.is_wide(*value) =>
            {
                return Err(unlegal(Unlegalized::Atomic));
            }
            InstKind::Call { callee, args } => match self.call(callee, args, result, at).map_err(unlegal)? {
                Some(parts) => parts,
                None => return Ok(()),
            },
            kind => {
                let kind = self.remapped(kind);
                let ty = result.map(|v| self.legalize.scalar(self.old.ty(v)));
                match self.b.push(kind, ty, at) {
                    Some(v) => Parts::One(v),
                    None => return Ok(()),
                }
            }
        };
        if let Some(v) = result {
            self.parts.insert(v, parts);
        }
        Ok(())
    }

    /// A conversion from or to a wide integer: an extension or truncation over limbs.
    fn cast(&mut self, op: CastOp, value: Value, to: Scalar, at: OriginId) -> Result<Parts, Unlegalized> {
        let count = self.legalize.limbs(to).map(usize::from);
        Ok(match (op, count) {
            (CastOp::ZExt | CastOp::SExt, Some(count)) => {
                let signed = op == CastOp::SExt;
                Parts::Limbs(if self.is_wide(value) {
                    let x = self.limbs(value);
                    self.limbwise(at).resized(&x, signed, count)
                } else {
                    let v = self.one(value);
                    self.limbwise(at).extended(v, signed, count)
                })
            }
            (CastOp::Trunc, Some(count)) => {
                let x = self.limbs(value);
                Parts::Limbs(self.limbwise(at).resized(&x, false, count))
            }
            (CastOp::Trunc, None) => {
                let low = self.limbs(value).low();
                Parts::One(if to == self.legalize.widest {
                    low
                } else {
                    self.b.cast(CastOp::Trunc, low, to, at)
                })
            }
            _ => return Err(Unlegalized::Float),
        })
    }

    /// `kind`, on no wide value, over the new values.
    pub(super) fn remapped(&self, kind: &InstKind) -> InstKind {
        let one = |v: &Value| self.one(*v);
        match kind {
            InstKind::Const { ty, bits } => InstKind::Const { ty: self.legalize.scalar(*ty), bits: *bits },
            InstKind::Binary { op, a, b } => InstKind::Binary { op: *op, a: one(a), b: one(b) },
            InstKind::Compare { cond, a, b } => InstKind::Compare { cond: *cond, a: one(a), b: one(b) },
            InstKind::Cast { op, value, to } => InstKind::Cast { op: *op, value: one(value), to: *to },
            InstKind::Select { cond, then, otherwise } => {
                InstKind::Select { cond: one(cond), then: one(then), otherwise: one(otherwise) }
            }
            InstKind::Load { ty, addr, access } => {
                InstKind::Load { ty: self.legalize.scalar(*ty), addr: one(addr), access: *access }
            }
            InstKind::Store { value, addr, access } => {
                InstKind::Store { value: one(value), addr: one(addr), access: *access }
            }
            InstKind::Rmw { op, addr, value, order } => {
                InstKind::Rmw { op: *op, addr: one(addr), value: one(value), order: *order }
            }
            InstKind::CompareExchange { addr, expected, new, success, failure } => {
                InstKind::CompareExchange {
                    addr: one(addr),
                    expected: one(expected),
                    new: one(new),
                    success: *success,
                    failure: *failure,
                }
            }
            InstKind::Offset { base, offset } => InstKind::Offset { base: one(base), offset: one(offset) },
            InstKind::SlotAddr(slot) => InstKind::SlotAddr(*slot),
            InstKind::Addr(address) => InstKind::Addr(*address),
            InstKind::Copy { to, from, bytes } => {
                InstKind::Copy { to: one(to), from: one(from), bytes: *bytes }
            }
            InstKind::Asm(..) => {
                let mut kind = kind.clone();
                kind.map_values(|v| self.one(v));
                kind
            }
            InstKind::Call { .. } => unreachable!("calls are rewritten apart"),
        }
    }
}
