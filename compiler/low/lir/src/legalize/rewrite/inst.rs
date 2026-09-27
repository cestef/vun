//! Instructions: each on a wide integer rewritten into a sequence over halves, each on a promoted
//! `f16` into one in `f32`, any other copied over the new values.

use super::super::{Unlegal, Unlegalized};
use super::{Parts, Rewrite};
use crate::{Access, BinOp, CastOp, Inst, InstKind, Value};

impl Rewrite<'_> {
    pub(super) fn inst(&mut self, inst: Inst) -> Result<(), Unlegal> {
        let data = &self.old.insts[inst];
        let (at, result) = (data.origin, data.result);
        let unlegal = |why| Unlegal { at, why };
        if let Some(parts) = self.half(&data.kind, at).map_err(unlegal)? {
            if let Some(v) = result {
                self.parts.insert(v, parts);
            }
            return Ok(());
        }
        let parts = match &data.kind {
            InstKind::Const { ty, bits } if Some(*ty) == self.wide => {
                Parts::Pair(self.halves(at).pair(*bits))
            }
            InstKind::Binary { op, a, b } if self.is_wide(*a) => {
                let (x, y) = (self.pair(*a), self.pair(*b));
                Parts::Pair(match op {
                    BinOp::Add => self.halves(at).add(x, y),
                    BinOp::Sub => self.halves(at).sub(x, y),
                    BinOp::Mul => self.halves(at).mul(x, y),
                    BinOp::And | BinOp::Or | BinOp::Xor => self.halves(at).bitwise(*op, x, y),
                    BinOp::Shl | BinOp::LShr | BinOp::AShr => self.halves(at).shift(*op, x, y.lo),
                    _ => self.divide(*op, x, y, at),
                })
            }
            InstKind::Compare { cond, a, b } if self.is_wide(*a) => {
                let (x, y) = (self.pair(*a), self.pair(*b));
                Parts::One(self.halves(at).compared(*cond, x, y))
            }
            InstKind::Cast { op, value, to } if self.is_wide(*value) || Some(*to) == self.wide => match op {
                CastOp::ZExt | CastOp::SExt => {
                    let v = self.one(*value);
                    Parts::Pair(self.halves(at).extended(v, *op == CastOp::SExt))
                }
                CastOp::Trunc => {
                    let lo = self.pair(*value).lo;
                    Parts::One(if *to == self.legalize.widest {
                        lo
                    } else {
                        self.b.cast(CastOp::Trunc, lo, *to, at)
                    })
                }
                _ => return Err(unlegal(Unlegalized::Float)),
            },
            InstKind::Select { cond, then, otherwise } if self.is_wide(*then) => {
                let (c, t, o) = (self.one(*cond), self.pair(*then), self.pair(*otherwise));
                Parts::Pair(self.halves(at).select_pair(c, t, o))
            }
            InstKind::Load { ty, addr, access } if Some(*ty) == self.wide => {
                if matches!(access, Access::Atomic(_)) {
                    return Err(unlegal(Unlegalized::Atomic));
                }
                let addr = self.one(*addr);
                Parts::Pair(self.load_pair(addr, *access, at))
            }
            InstKind::Store { value, addr, access } if self.is_wide(*value) => {
                if matches!(access, Access::Atomic(_)) {
                    return Err(unlegal(Unlegalized::Atomic));
                }
                let (value, addr) = (self.pair(*value), self.one(*addr));
                self.store_pair(value, addr, *access, at);
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
            InstKind::Asm(_) => {
                let mut kind = kind.clone();
                kind.map_values(|v| self.one(v));
                kind
            }
            InstKind::Call { .. } => unreachable!("calls are rewritten apart"),
        }
    }
}
