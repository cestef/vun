//! Places as addresses: a local's slot, then each projection in turn. `.*` loads the address a
//! borrow, pointer or owner holds (and a slice borrow's length); a field, position or payload
//! item is an offset its type's shape gives; an item at an index is checked against the count
//! of items and scaled by their stride.

use vun_codec::{Decode, Encode};
use vun_lir::{Access, BinOp, CastOp, Cond, Scalar, Value};
use vun_mir::{Place, Projection};
use vun_sema::layout::Parts;
use vun_sema::member::Members;
use vun_sema_ty::trap::Reason;
use vun_sema_ty::{Ty, TyKind};

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

/// A place reached: its address, its type, and for the items of a slice how many there are.
#[derive(Clone, Copy, Encode, Decode)]
pub(crate) struct Reached {
    pub(crate) addr: Value,
    pub(crate) ty: Ty,
    pub(crate) len: Option<Value>,
}

impl FnLower<'_, '_, '_> {
    /// The address of `place` and its type; no address for a place of size 0.
    pub(crate) fn place(&mut self, place: &Place) -> Result<(Option<Value>, Ty), Unlowered> {
        if let Some(reached) = self.reach(place)? {
            return Ok((Some(reached.addr), reached.ty));
        }
        let ty = place.ty(self.l.types.cx, &self.body().locals);
        Ok((None, ty.ok_or(Unlowered::unsupported(self.at, Form::Projection))?))
    }

    pub(crate) fn reach(&mut self, place: &Place) -> Result<Option<Reached>, Unlowered> {
        let ty = self.ty(place.local)?;
        let Some(addr) = self.local(place.local) else { return Ok(None) };
        let mut reached = Reached { addr, ty, len: None };
        for projection in &place.projection {
            reached = self.project(reached, projection)?;
        }
        Ok(Some(reached))
    }

    /// The address `offset` bytes past `addr`.
    pub(crate) fn at_offset(&mut self, addr: Value, offset: u64) -> Value {
        if offset == 0 {
            return addr;
        }
        let word = self.l.types.word;
        let offset = self.b.constant(word, u128::from(offset), self.at);
        self.b.offset(addr, offset, self.at)
    }

    fn project(&mut self, from: Reached, projection: &Projection) -> Result<Reached, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Projection);
        let cx = self.l.types.cx;
        // Slices have no shape of their own: only what their parts need is computed.
        let shape = || self.l.types.shape(from.ty, self.at);
        match projection {
            Projection::Deref => {
                // An owner reads through the pointer field marked `#[kind.owns]`.
                if let Some(members) = cx.get::<Members>(&from.ty)
                    && let Some((field, pointee)) = members.deref()
                {
                    let (offset, _) = self.l.types.field(from.ty, field).ok_or_else(unsupported)?;
                    let field = self.at_offset(from.addr, offset);
                    let addr = self.b.load(Scalar::Ptr, field, Access::Plain, self.at);
                    return Ok(Reached { addr, ty: pointee, len: None });
                }
                let pointee = from.ty.pointee().ok_or_else(unsupported)?;
                let addr = self.b.load(Scalar::Ptr, from.addr, Access::Plain, self.at);
                let len = match pointee.slice_items() {
                    Some(_) => {
                        let word = self.l.types.word;
                        let at = self.at_offset(from.addr, word.bytes(self.l.types.pointer_bits()));
                        Some(self.b.load(word, at, Access::Plain, self.at))
                    }
                    None => None,
                };
                Ok(Reached { addr, ty: pointee, len })
            }
            Projection::Field(name) => {
                let index = match &*from.ty {
                    TyKind::Record(fields) => fields.iter().position(|(n, _)| n == name),
                    _ => cx
                        .get::<Members>(&from.ty)
                        .and_then(|m| m.fields.iter().position(|f| f.name == *name)),
                };
                let parts = shape()?.parts.clone();
                self.part(from.ty, &parts, index.ok_or_else(unsupported)?, from.addr)
            }
            Projection::Position(at) => {
                let parts = shape()?.parts.clone();
                self.part(from.ty, &parts, *at, from.addr)
            }
            Projection::Payload { variant, at } => {
                let members = cx.get::<Members>(&from.ty).ok_or_else(unsupported)?;
                let index =
                    members.variants.iter().position(|v| v.name == *variant).ok_or_else(unsupported)?;
                let ty = members.variants[index].payload.get(*at).and_then(|t| t.as_ref().ok().copied());
                let shape = shape()?;
                let Parts::Enum { payloads, .. } = &shape.parts else { return Err(unsupported()) };
                let offset = payloads.get(index).and_then(|p| p.get(*at)).ok_or_else(unsupported)?;
                Ok(Reached {
                    addr: self.at_offset(from.addr, *offset),
                    ty: ty.ok_or_else(unsupported)?,
                    len: None,
                })
            }
            Projection::Index(local) => self.item(from, *local),
        }
    }

    /// The part at `index` of a value of type `ty` laid out as `parts`, at `addr`.
    fn part(&mut self, ty: Ty, parts: &Parts, index: usize, addr: Value) -> Result<Reached, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Projection);
        let Parts::Sequence(offsets) = parts else { return Err(unsupported()) };
        let part = self.l.types.parts(ty).and_then(|p| p.get(index).copied()).ok_or_else(unsupported)?;
        let offset = *offsets.get(index).ok_or_else(unsupported)?;
        Ok(Reached { addr: self.at_offset(addr, offset), ty: part, len: None })
    }

    /// The item at the index held in `local` of the array or slice reached, trapping past the
    /// last one.
    fn item(&mut self, from: Reached, local: vun_mir::Local) -> Result<Reached, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Projection);
        let word = self.l.types.word;
        let (item, len) = match (from.ty.as_array(), from.ty.slice_items(), from.len) {
            (Some((len, item)), ..) => {
                let len = len.to_u128().ok_or_else(unsupported)?;
                (item, self.b.constant(word, len, self.at))
            }
            (None, Some(item), Some(len)) => (item, len),
            _ => return Err(unsupported()),
        };
        let stride = self.l.types.shape(item, self.at)?.layout.size;
        let index_ty = self.ty(local)?;
        // An index of an empty type, the loop variable over no items, is never held: nothing runs
        // that reaches this item, so it is placed at the start.
        if index_ty.is_empty() {
            return Ok(Reached { addr: from.addr, ty: item, len: None });
        }
        // Bounds select a run of the items.
        if index_ty.bounds().is_some() {
            let bounds = self.local(local).ok_or_else(unsupported)?;
            return self.run(from, item, stride, len, bounds, index_ty);
        }
        let (index_int, signed) = self.l.types.int(index_ty).ok_or_else(unsupported)?;
        let addr = self.local(local).ok_or_else(unsupported)?;
        let index = self.b.load(index_int, addr, Access::Plain, self.at);
        let index = self.word_index(index, index_int, signed)?;
        let inside = self.b.compare(Cond::ULt, index, len, self.at);
        self.check(inside, &Reason::Bounds)?;
        let stride = self.b.constant(word, u128::from(stride), self.at);
        let offset = self.b.binary(BinOp::Mul, index, stride, self.at);
        Ok(Reached { addr: self.b.offset(from.addr, offset, self.at), ty: item, len: None })
    }

    /// `value`, an index held in `int`, as a word to compare with a length: one past every word,
    /// which no length reaches, traps rather than losing its high bits.
    pub(crate) fn word_index(&mut self, value: Value, int: Scalar, signed: bool) -> Result<Value, Unlowered> {
        let word = self.l.types.word;
        let bits = word.bits(self.l.types.pointer_bits());
        if self.wider_than_word(int) {
            // A negative index is past every word too, read without its sign.
            let words = self.b.constant(int, 1 << bits, self.at);
            let within = self.b.compare(Cond::ULt, value, words, self.at);
            self.check(within, &Reason::Bounds)?;
        }
        Ok(self.resize(value, int, signed, word))
    }

    /// Whether an integer held in `int` may be past every word.
    pub(crate) fn wider_than_word(&self, int: Scalar) -> bool {
        let bits = |s: Scalar| s.bits(self.l.types.pointer_bits());
        bits(int) > bits(self.l.types.word)
    }

    /// `value`, an integer of type `from` (signed or not), as an integer of type `to`.
    pub(crate) fn resize(&mut self, value: Value, from: Scalar, signed: bool, to: Scalar) -> Value {
        let bits = |s: Scalar| s.bits(self.l.types.pointer_bits());
        match bits(from).cmp(&bits(to)) {
            std::cmp::Ordering::Equal => value,
            std::cmp::Ordering::Less => {
                let op = if signed { CastOp::SExt } else { CastOp::ZExt };
                self.b.cast(op, value, to, self.at)
            }
            std::cmp::Ordering::Greater => self.b.cast(CastOp::Trunc, value, to, self.at),
        }
    }
}
