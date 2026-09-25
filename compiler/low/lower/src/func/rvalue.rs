//! What a statement computes into its place: a copy or conversion, a condition, a borrow, an
//! aggregate built part by part, a call, or an integer brought into the place's type.

use vun_desugar::Narrowing;
use vun_lir::{Access, BinOp, Cond, Scalar, Value};
use vun_mir::{Aggregate, Operand, Place, Rvalue};
use vun_sema::int::Integer;
use vun_sema::layout::Parts;
use vun_sema::member::Members;
use vun_sema::ty::{Ty, Value as Constant};

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// Computes `rvalue` into `dest`, of type `ty`; nothing is stored for a value of size 0.
    pub(crate) fn rvalue(&mut self, rvalue: &Rvalue, dest: Option<Value>, ty: Ty) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move |form| Unlowered::unsupported(at, form);
        match rvalue {
            Rvalue::Use(operand) => self.write(operand, dest, ty),
            Rvalue::Not { operand } => {
                let holds = self.holds(operand)?;
                let zero = self.b.constant(Scalar::I8, 0, self.at);
                let fails = self.b.compare(Cond::Eq, holds, zero, self.at);
                self.condition(fails, dest, ty)
            }
            Rvalue::IsVariant { place, variant } => {
                let holds = match self.reach(place)? {
                    Some(r) => self.is_variant(r.addr, r.ty, *variant)?,
                    None => self.b.constant(Scalar::I8, 1, self.at),
                };
                self.condition(holds, dest, ty)
            }
            Rvalue::Ref { place, .. } => self.borrow(place, dest, ty),
            Rvalue::Aggregate { kind, parts } => self.aggregate(kind, parts, dest, ty),
            Rvalue::Call { callee, args } => self.call(callee, args, dest, ty),
            Rvalue::Narrow { operand, narrowing, .. } => self.narrow(operand, *narrowing, dest, ty),
            Rvalue::Repeat { operand, count } => {
                let (Some(dest), Some((_, item))) = (dest, ty.as_array()) else { return Ok(()) };
                let Constant::Int(count) = count else { return Err(unsupported(Form::Constant)) };
                let Parts::Array { stride, .. } = self.l.types.shape(ty, self.at)?.parts else {
                    return Err(unsupported(Form::Projection));
                };
                for i in 0..count.to_u128().and_then(|c| u64::try_from(c).ok()).unwrap_or_default() {
                    let at = self.at_offset(dest, i * stride);
                    self.write(operand, Some(at), item)?;
                }
                Ok(())
            }
            Rvalue::Range { op, lo, hi } => self.range(*op, lo, hi.as_ref(), dest, ty),
            Rvalue::Resume { frame, .. } => self.drive(frame, dest, super::coroutine::Mode::Resume),
            Rvalue::End { frame } => self.drive(frame, None, super::coroutine::Mode::End),
            Rvalue::Comptime { .. } | Rvalue::Quote { .. } | Rvalue::Splice { .. } => {
                Err(unsupported(Form::Quote))
            }
            Rvalue::Dead => Ok(()),
        }
    }

    /// The truth of the condition `operand`: 1 or 0, an `i8`.
    pub(crate) fn holds(&mut self, operand: &Operand) -> Result<Value, Unlowered> {
        let (ty, truth, _) = self.truth()?;
        let addr = match operand {
            Operand::Place(place) => self.reach(place)?.map(|r| r.addr),
            Operand::Const(value) => {
                let temp = self.temp(ty)?;
                if let Some(temp) = temp {
                    self.write_const(value, temp, ty)?;
                }
                temp
            }
        };
        let addr = addr.ok_or(Unlowered::unsupported(self.at, Form::Conversion))?;
        self.is_variant(addr, ty, truth)
    }

    /// `&place` into `dest`: its address, and for a borrow of a slice the count of its items.
    fn borrow(&mut self, place: &Place, dest: Option<Value>, ty: Ty) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        let (addr, pointee, len) = if let Some(r) = self.reach(place)? {
            (r.addr, r.ty, r.len)
        } else {
            // A place of size 0 has no storage; any address of its alignment reaches it.
            let slot = self.b.slot(1, 1, self.at);
            let pointee = place
                .ty(self.l.types.cx, &self.body().locals)
                .ok_or(Unlowered::unsupported(self.at, Form::Projection))?;
            (self.b.slot_addr(slot, self.at), pointee, None)
        };
        self.b.store(addr, dest, Access::Plain, self.at);
        if ty.borrowed().and_then(|t| t.slice_items()).is_some() {
            let word = self.l.types.word;
            let len = match (len, pointee.as_array()) {
                (Some(len), _) => len,
                (None, Some((n, _))) => self.b.constant(word, n.to_u128().unwrap_or_default(), self.at),
                (None, None) => return Err(Unlowered::unsupported(self.at, Form::Conversion)),
            };
            let at = self.at_offset(dest, word.bytes(self.l.types.pointer_bits()));
            self.b.store(len, at, Access::Plain, self.at);
        }
        Ok(())
    }

    pub(super) fn aggregate(
        &mut self,
        kind: &Aggregate,
        parts: &[Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Projection);
        let Some(dest) = dest else { return Ok(()) };
        let shape = self.l.types.shape(ty, self.at)?;
        match (kind, &shape.parts) {
            (Aggregate::Variant { name, .. }, Parts::Enum { payloads, .. }) => {
                let members = self.l.types.cx.get::<Members>(&ty).ok_or_else(unsupported)?;
                let index = members.variants.iter().position(|v| v.name == *name).ok_or_else(unsupported)?;
                for ((part, item), offset) in
                    parts.iter().zip(&members.variants[index].payload).zip(&payloads[index])
                {
                    let at = self.at_offset(dest, *offset);
                    self.write(part, Some(at), *item.as_ref().map_err(|_| unsupported())?)?;
                }
                self.set_variant(dest, ty, *name)
            }
            (Aggregate::Array, Parts::Array { stride, .. }) => {
                let (_, item) = ty.as_array().ok_or_else(unsupported)?;
                for (i, part) in (0..).zip(parts) {
                    let at = self.at_offset(dest, i * stride);
                    self.write(part, Some(at), item)?;
                }
                Ok(())
            }
            (Aggregate::Struct { fields, .. }, Parts::Sequence(offsets)) => {
                let members = self.l.types.cx.get::<Members>(&ty).ok_or_else(unsupported)?;
                for (name, part) in fields.iter().zip(parts) {
                    let index =
                        members.fields.iter().position(|f| f.name == *name).ok_or_else(unsupported)?;
                    let field = *members.fields[index].ty.as_ref().map_err(|_| unsupported())?;
                    let at = self.at_offset(dest, offsets[index]);
                    self.write(part, Some(at), field)?;
                }
                Ok(())
            }
            (Aggregate::Tuple | Aggregate::Closure { .. }, Parts::Sequence(offsets)) => {
                let types = self.l.types.parts(ty).ok_or_else(unsupported)?;
                for ((part, item), offset) in parts.iter().zip(types).zip(offsets) {
                    let at = self.at_offset(dest, *offset);
                    self.write(part, Some(at), item)?;
                }
                Ok(())
            }
            (_, Parts::Whole) if parts.is_empty() => Ok(()),
            _ => Err(unsupported()),
        }
    }

    /// `operand` brought into the integer type `ty` (spec 04, rule 9): trapping outside it,
    /// reduced modulo its size, or clamped to its bounds.
    pub(crate) fn narrow(
        &mut self,
        operand: &Operand,
        narrowing: Narrowing,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Conversion);
        let Some(dest) = dest else { return Ok(()) };
        let from = self.operand_ty(operand).ok_or_else(unsupported)?;
        let temp = self.temp(from)?.ok_or_else(unsupported)?;
        self.write(operand, Some(temp), from)?;
        let (source, signed) = self.l.types.int(from).ok_or_else(unsupported)?;
        let (target, _) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let (range, own) = (ty.interval().ok_or_else(unsupported)?, from.interval().ok_or_else(unsupported)?);
        let bound = |e: &vun_sema::int::Extended| e.finite().and_then(Integer::to_i128);
        let (lo, hi) =
            (bound(range.lo()).ok_or_else(unsupported)?, bound(range.hi()).ok_or_else(unsupported)?);
        // Only a bound inside the source's range cuts it, and such a bound fits the source's
        // storage: checks and clamps compare there, and nothing widens past both storages.
        let (cuts_lo, cuts_hi) = (range.lo() > own.lo(), range.hi() < own.hi());
        let (ge, le, lt, gt) = if signed {
            (Cond::SGe, Cond::SLe, Cond::SLt, Cond::SGt)
        } else {
            (Cond::UGe, Cond::ULe, Cond::ULt, Cond::UGt)
        };
        let x = self.b.load(source, temp, Access::Plain, self.at);
        let value = match narrowing {
            Narrowing::To => {
                let mut inside = None;
                for (cuts, cond, n) in [(cuts_lo, ge, lo), (cuts_hi, le, hi)] {
                    if cuts {
                        let n = self.b.constant(source, source.truncate(n), self.at);
                        let holds = self.b.compare(cond, x, n, self.at);
                        inside = Some(match inside {
                            Some(both) => self.b.binary(BinOp::And, both, holds, self.at),
                            None => holds,
                        });
                    }
                }
                if let Some(inside) = inside {
                    self.check(inside)?;
                }
                self.resize(x, source, signed, target)
            }
            Narrowing::Sat => {
                let mut x = x;
                for (cuts, cond, n) in [(cuts_lo, lt, lo), (cuts_hi, gt, hi)] {
                    if cuts {
                        let n = self.b.constant(source, source.truncate(n), self.at);
                        let past = self.b.compare(cond, x, n, self.at);
                        let kind = vun_lir::InstKind::Select { cond: past, then: n, otherwise: x };
                        x = self.b.push(kind, Some(source), self.at).expect("a select has a value");
                    }
                }
                self.resize(x, source, signed, target)
            }
            // Modular reduction into a range whose size is a power of two, from its low bound,
            // in the wider of both storages, whose wrapping keeps the low bits exact.
            Narrowing::Wrap => {
                let size =
                    hi.checked_sub(lo).and_then(|d| d.checked_add(1)).and_then(|n| u128::try_from(n).ok());
                if !size.is_some_and(u128::is_power_of_two) {
                    return Err(unsupported());
                }
                let bits = |s: Scalar| s.bits(self.l.types.pointer_bits());
                let wide = if bits(target) > bits(source) { target } else { source };
                let x = self.resize(x, source, signed, wide);
                let (low, mask) = (
                    self.b.constant(wide, wide.truncate(lo), self.at),
                    self.b.constant(wide, wide.truncate(hi - lo), self.at),
                );
                let shifted = self.b.binary(BinOp::Sub, x, low, self.at);
                let reduced = self.b.binary(BinOp::And, shifted, mask, self.at);
                let value = self.b.binary(BinOp::Add, reduced, low, self.at);
                self.resize(value, wide, signed, target)
            }
        };
        self.b.store(value, dest, Access::Plain, self.at);
        Ok(())
    }
}
