//! Moving values between places of related types: an integer into a wider or narrower storage
//! of a range holding it, a vector into the array of its lanes and back, a borrow of an array
//! into a borrow of a slice, a borrow into one of
//! another mode, a variant into an enum of the same constructor laid out apart
//! (`Result(never, E)` into `Result(u64, E)`), and arrays and tuples item by item.

use vun_lir::{Access, Cond, Scalar, Target, Terminator, Value};
use vun_mir::Operand;
use vun_sema::layout::Parts;
use vun_sema::member::Members;
use vun_sema_ty::{Ty, TyKind};

use super::FnLower;
use crate::repr::Repr;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// Writes `operand` as a value of type `ty` at `dest`; nothing for a value of size 0.
    pub(crate) fn write(&mut self, operand: &Operand, dest: Option<Value>, ty: Ty) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        match operand {
            Operand::Const(value) => self.write_const(value, dest, ty),
            Operand::Place(place) => match self.reach(place)? {
                Some(from) => self.convert(from.addr, from.ty, dest, ty),
                None => Ok(()),
            },
        }
    }

    /// The value of type `ty` at `addr`, as a scalar.
    pub(crate) fn load_scalar(&mut self, addr: Value, ty: Ty) -> Result<Value, Unlowered> {
        match self.l.types.repr(ty, self.at)? {
            Repr::Scalar(s) => Ok(self.b.load(s, addr, Access::Plain, self.at)),
            _ => Err(Unlowered::unsupported(self.at, Form::Conversion)),
        }
    }

    /// Copies the value of type `ty` at `from` to `to`.
    pub(crate) fn copy_value(&mut self, from: Value, to: Value, ty: Ty) -> Result<(), Unlowered> {
        match self.l.types.repr(ty, self.at)? {
            Repr::Zero => {}
            Repr::Scalar(s) => {
                let value = self.b.load(s, from, Access::Plain, self.at);
                self.b.store(value, to, Access::Plain, self.at);
            }
            Repr::Memory(layout) => self.b.copy(to, from, layout.size, self.at),
        }
        Ok(())
    }

    /// Copies the value of type `from` at `src` to `dest`, as a value of type `to`.
    pub(crate) fn convert(&mut self, src: Value, from: Ty, dest: Value, to: Ty) -> Result<(), Unlowered> {
        let types = self.l.types;
        if from == to {
            return self.copy_value(src, dest, to);
        }
        if let (Some((fi, signed)), Some((ti, _))) = (types.int(from), types.int(to)) {
            let value = self.b.load(fi, src, Access::Plain, self.at);
            let value = self.resize(value, fi, signed, ti);
            self.b.store(value, dest, Access::Plain, self.at);
            return Ok(());
        }
        // A borrow of `[N]T` as a borrow of `[T]`: its address and `N`.
        if let (Some(array), Some(slice)) = (from.borrowed(), to.borrowed())
            && let (Some((len, _)), Some(_)) = (array.as_array(), slice.slice_items())
        {
            let addr = self.b.load(Scalar::Ptr, src, Access::Plain, self.at);
            self.b.store(addr, dest, Access::Plain, self.at);
            let word = types.word;
            let len = self.b.constant(word, len.to_u128().unwrap_or_default(), self.at);
            let at = self.at_offset(dest, word.bytes(types.pointer_bits()));
            self.b.store(len, at, Access::Plain, self.at);
            return Ok(());
        }
        // A vector as the array of its lanes, and back: laid out alike.
        if let (Some(a), Some(b)) = (from.repeated(), to.repeated())
            && a == b
        {
            return self.copy_value(src, dest, to);
        }
        // Arrays of one length, item by item.
        if let (Some((n, a)), Some((m, b))) = (from.as_array(), to.as_array())
            && n == m
        {
            let (sa, sb) = (types.shape(a, self.at)?.layout.size, types.shape(b, self.at)?.layout.size);
            let len = n
                .to_u128()
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(Unlowered::unsupported(self.at, Form::Conversion))?;
            let len = self.b.constant(self.l.types.word, u128::from(len), self.at);
            return self.each(len, |this, i| {
                let (at_a, at_b) = (this.scaled(src, i, sa), this.scaled(dest, i, sb));
                this.convert(at_a, a, at_b, b)
            });
        }
        // Tuples of as many items, item by item.
        if let (Some(a), Some(b)) = (from.as_tuple(), to.as_tuple())
            && a.len() == b.len()
        {
            let (a, b) = (a.to_vec(), b.to_vec());
            let (sa, sb) = (types.shape(from, self.at)?, types.shape(to, self.at)?);
            let (Parts::Sequence(oa), Parts::Sequence(ob)) = (&sa.parts, &sb.parts) else {
                return Err(Unlowered::unsupported(self.at, Form::Conversion));
            };
            for (((&a, &b), &oa), &ob) in a.iter().zip(&b).zip(oa).zip(ob) {
                let (at_a, at_b) = (self.at_offset(src, oa), self.at_offset(dest, ob));
                self.convert(at_a, a, at_b, b)?;
            }
            return Ok(());
        }
        if let (TyKind::Nominal(a), TyKind::Nominal(b)) = (&*from, &*to)
            && a.literal == b.literal
        {
            return self.variants(src, from, dest, to);
        }
        // A borrow as a borrow of the same pointee in another mode, `&mut T` as `&T` or
        // `&static T` as `&T`: held alike, whatever the pointee.
        if let (Some((_, _, a)), Some((_, _, b))) = (from.borrowing(), to.borrowing())
            && a == b
        {
            return self.copy_value(src, dest, to);
        }
        // Types held alike.
        match (types.repr(from, self.at)?, types.repr(to, self.at)?) {
            (Repr::Scalar(a), Repr::Scalar(b)) if a == b => self.copy_value(src, dest, to),
            _ => Err(Unlowered::unsupported(self.at, Form::Conversion)),
        }
    }

    /// Runs `body` for each index below `len`, a word, in a loop.
    pub(crate) fn each(
        &mut self,
        len: Value,
        mut body: impl FnMut(&mut Self, Value) -> Result<(), Unlowered>,
    ) -> Result<(), Unlowered> {
        let word = self.l.types.word;
        let (head, run, done) =
            (self.b.block(&[word], self.at), self.b.block(&[], self.at), self.b.block(&[], self.at));
        let zero = self.b.constant(word, 0, self.at);
        self.b.terminate(Terminator::Jump(Target { block: head, args: vec![zero] }));
        self.b.switch_to(head);
        let i = self.b.params(head)[0];
        let more = self.b.compare(Cond::ULt, i, len, self.at);
        let to = |block| Target { block, args: Vec::new() };
        self.b.terminate(Terminator::Branch { cond: more, then: to(run), otherwise: to(done) });
        self.b.switch_to(run);
        body(self, i)?;
        let one = self.b.constant(word, 1, self.at);
        let next = self.b.binary(vun_lir::BinOp::Add, i, one, self.at);
        self.b.terminate(Terminator::Jump(Target { block: head, args: vec![next] }));
        self.b.switch_to(done);
        Ok(())
    }

    /// The address of the item at the index `i`, a word, of items `stride` bytes apart from
    /// `base`.
    pub(crate) fn scaled(&mut self, base: Value, i: Value, stride: u64) -> Value {
        let stride = self.b.constant(self.l.types.word, u128::from(stride), self.at);
        let offset = self.b.binary(vun_lir::BinOp::Mul, i, stride, self.at);
        self.b.offset(base, offset, self.at)
    }

    /// Copies the enum of type `from` at `src` into `dest` as an enum of type `to`: its variant,
    /// and each payload item converted.
    fn variants(&mut self, src: Value, from: Ty, dest: Value, to: Ty) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Conversion);
        let cx = self.l.types.cx;
        let (source, target) = (
            cx.get::<Members>(&from).ok_or_else(unsupported)?,
            cx.get::<Members>(&to).ok_or_else(unsupported)?,
        );
        let join = self.b.block(&[], self.at);
        let to_block = |block| Target { block, args: Vec::new() };
        for variant in &source.variants {
            let items: Vec<Ty> = variant
                .payload
                .iter()
                .map(|p| p.as_ref().ok().copied())
                .collect::<Option<_>>()
                .ok_or_else(unsupported)?;
            // A variant carrying a value of an empty type never occurs.
            if items.iter().any(|t| t.is_empty()) {
                continue;
            }
            let into = target.variants.iter().find(|v| v.name == variant.name).ok_or_else(unsupported)?;
            let into: Vec<Ty> = into
                .payload
                .iter()
                .map(|p| p.as_ref().ok().copied())
                .collect::<Option<_>>()
                .ok_or_else(unsupported)?;
            let (this, next) = (self.b.block(&[], self.at), self.b.block(&[], self.at));
            let holds = self.is_variant(src, from, variant.name)?;
            self.b.terminate(Terminator::Branch {
                cond: holds,
                then: to_block(this),
                otherwise: to_block(next),
            });
            self.b.switch_to(this);
            for (at, (&item, &into)) in items.iter().zip(&into).enumerate() {
                let (a, b) =
                    (self.payload(src, from, variant.name, at)?, self.payload(dest, to, variant.name, at)?);
                self.convert(a, item, b, into)?;
            }
            self.set_variant(dest, to, variant.name)?;
            self.b.terminate(Terminator::Jump(to_block(join)));
            self.b.switch_to(next);
        }
        self.b.terminate(Terminator::Unreachable);
        self.b.switch_to(join);
        Ok(())
    }
}
