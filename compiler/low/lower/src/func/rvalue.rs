//! What a statement computes into its place: a copy or conversion, a condition, a borrow, an
//! aggregate built part by part, a call, or an integer brought into the place's type.

use vun_desugar::{Form as Syntax, Narrowing};
use vun_intern::Symbol;
use vun_lir::{Access, BinOp, Cond, Scalar, Target, Terminator, Value};
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
            Rvalue::Resume { .. } | Rvalue::End { .. } => Err(unsupported(Form::Frame)),
            Rvalue::Comptime { .. } | Rvalue::Quote { .. } => Err(unsupported(Form::Quote)),
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

    /// The type of conditions, the variant holding, and the other one (the table `forms`).
    fn truth(&self) -> Result<(Ty, Symbol, Symbol), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Constant);
        let condition =
            self.l.env.form(Syntax::Condition).ok().and_then(|v| v.as_type()).ok_or_else(unsupported)?;
        let Ok(Constant::Variant { name: truth, .. }) = self.l.env.form(Syntax::Truth) else {
            return Err(unsupported());
        };
        let members = self.l.types.cx.get::<Members>(&condition).ok_or_else(unsupported)?;
        let other = members.variants.iter().find(|v| v.name != truth).ok_or_else(unsupported)?.name;
        Ok((condition, truth, other))
    }

    /// Writes into `dest`, a condition, whether `holds` is not 0.
    pub(crate) fn condition(&mut self, holds: Value, dest: Option<Value>, ty: Ty) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        let (_, truth, other) = self.truth()?;
        let (yes, no, join) =
            (self.b.block(&[], self.at), self.b.block(&[], self.at), self.b.block(&[], self.at));
        let to = |block| Target { block, args: Vec::new() };
        self.b.terminate(Terminator::Branch { cond: holds, then: to(yes), otherwise: to(no) });
        for (block, variant) in [(yes, truth), (no, other)] {
            self.b.switch_to(block);
            self.set_variant(dest, ty, variant)?;
            self.b.terminate(Terminator::Jump(to(join)));
        }
        self.b.switch_to(join);
        Ok(())
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

    fn aggregate(
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
        let range = ty.interval().ok_or_else(unsupported)?;
        let bound = |e: &vun_sema::int::Extended| e.finite().and_then(Integer::to_i128);
        let (lo, hi) =
            (bound(range.lo()).ok_or_else(unsupported)?, bound(range.hi()).ok_or_else(unsupported)?);
        // Modular reduction masks: the range must hold a power of two of integers.
        let size = hi.checked_sub(lo).and_then(|d| d.checked_add(1)).and_then(|n| u128::try_from(n).ok());
        if narrowing == Narrowing::Wrap && !size.is_some_and(u128::is_power_of_two) {
            return Err(unsupported());
        }
        let x = self.b.load(source, temp, Access::Plain, self.at);
        let x = self.resize(x, source, signed, Scalar::I128);
        #[expect(clippy::cast_sign_loss, reason = "constants are two's complement bits")]
        let (lo, hi) = (
            self.b.constant(Scalar::I128, lo as u128, self.at),
            self.b.constant(Scalar::I128, hi as u128, self.at),
        );
        let value = match narrowing {
            Narrowing::To => {
                let above = self.b.compare(Cond::SGe, x, lo, self.at);
                let below = self.b.compare(Cond::SLe, x, hi, self.at);
                let inside = self.b.binary(BinOp::And, above, below, self.at);
                self.check(inside)?;
                x
            }
            Narrowing::Sat => {
                let low = self.b.compare(Cond::SLt, x, lo, self.at);
                let x = self.b.push(
                    vun_lir::InstKind::Select { cond: low, then: lo, otherwise: x },
                    Some(Scalar::I128),
                    self.at,
                );
                let x = x.expect("a select has a value");
                let high = self.b.compare(Cond::SGt, x, hi, self.at);
                let x = self.b.push(
                    vun_lir::InstKind::Select { cond: high, then: hi, otherwise: x },
                    Some(Scalar::I128),
                    self.at,
                );
                x.expect("a select has a value")
            }
            // Modular reduction into a range whose size is a power of two, from its low bound.
            Narrowing::Wrap => {
                let size = self.b.binary(BinOp::Sub, hi, lo, self.at);
                let shifted = self.b.binary(BinOp::Sub, x, lo, self.at);
                let reduced = self.b.binary(BinOp::And, shifted, size, self.at);
                self.b.binary(BinOp::Add, reduced, lo, self.at)
            }
        };
        let value = self.resize(value, Scalar::I128, true, target);
        self.b.store(value, dest, Access::Plain, self.at);
        Ok(())
    }
}
