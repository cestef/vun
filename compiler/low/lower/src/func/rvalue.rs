//! What a statement computes into its place: a copy or conversion, a condition, a borrow, an
//! aggregate built part by part, a call, or an integer brought into the place's type.

use vun_lir::{Access, Cond, Scalar, Value};
use vun_mir::{Aggregate, Operand, Place, Rvalue};
use vun_sema::layout::Parts;
use vun_sema::member::Members;
use vun_sema_ty::{Ty, Value as Constant};

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
                // A loop over the items, whatever their count: `[0; 4096]` is a few instructions.
                let count = count.to_u128().ok_or(unsupported(Form::Constant))?;
                let len = self.b.constant(self.l.types.word, count, self.at);
                self.each(len, |this, i| {
                    let at = this.scaled(dest, i, stride);
                    this.write(operand, Some(at), item)
                })
            }
            Rvalue::Range { op, lo, hi } => self.range(*op, lo, hi.as_ref(), dest, ty),
            Rvalue::Resume { frame, .. } => self.drive(frame, dest, super::coroutine::Mode::Resume),
            Rvalue::End { frame } => self.drive(frame, None, super::coroutine::Mode::End),
            Rvalue::Comptime { .. } | Rvalue::Quote { .. } | Rvalue::Splice { .. } => {
                Err(unsupported(Form::Quote))
            }
            Rvalue::Asm(asm) => self.asm(asm),
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
            (Aggregate::Struct { fields, .. }, Parts::Sequence(_)) => {
                for (name, part) in fields.iter().zip(parts) {
                    let (offset, field) = self.l.types.field(ty, *name).ok_or_else(unsupported)?;
                    let at = self.at_offset(dest, offset);
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
}
