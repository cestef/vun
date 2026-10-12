//! The variant of a value: telling it, its position, its parts and writing it. A struct is one
//! variant, its fields its parts (spec 03, section 6).

use vun_intern::Symbol;
use vun_lir::{Access, Cond, Scalar, Value};
use vun_sema::layout::{Parts, Shape, Tag};
use vun_sema::member::Members;
use vun_sema_ty::Ty;

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// The index of `variant` among those of the enum `ty`, and the enum's shape.
    fn variant(&self, ty: Ty, variant: Symbol) -> Result<(usize, std::sync::Arc<Shape>), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Projection);
        let members = self.l.types.cx.get::<Members>(&ty).ok_or_else(unsupported)?;
        let index = members.variants.iter().position(|v| v.name == variant).ok_or_else(unsupported)?;
        Ok((index, self.l.types.shape(ty, self.at)?))
    }

    /// The address of the payload item `at` of `variant` of the enum of type `ty` at `addr`.
    pub(crate) fn payload(
        &mut self,
        addr: Value,
        ty: Ty,
        variant: Symbol,
        at: usize,
    ) -> Result<Value, Unlowered> {
        let (index, shape) = self.variant(ty, variant)?;
        let Parts::Enum { payloads, .. } = &shape.parts else {
            return Err(Unlowered::unsupported(self.at, Form::Projection));
        };
        let offset = *payloads[index].get(at).ok_or(Unlowered::unsupported(self.at, Form::Projection))?;
        Ok(self.at_offset(addr, offset))
    }

    /// Whether the enum of type `ty` at `addr` is `variant`: 1 or 0, an `i8`.
    pub(crate) fn is_variant(&mut self, addr: Value, ty: Ty, variant: Symbol) -> Result<Value, Unlowered> {
        let (index, shape) = self.variant(ty, variant)?;
        let Parts::Enum { tag, .. } = &shape.parts else {
            return Err(Unlowered::unsupported(self.at, Form::Projection));
        };
        let (offset, bytes, pattern, cond) = match tag {
            Tag::Alone => return Ok(self.b.constant(Scalar::I8, 1, self.at)),
            Tag::Index { bytes, .. } => {
                let pattern = tag.pattern(index).ok_or(Unlowered::unsupported(self.at, Form::Projection))?;
                (0, *bytes, pattern, Cond::Eq)
            }
            Tag::Niche { empty, offset, bytes, value } => {
                (*offset, *bytes, *value, if *empty == index { Cond::Eq } else { Cond::Ne })
            }
        };
        let int = Scalar::int(u32::try_from(bytes * 8).unwrap_or_default())
            .ok_or(Unlowered::unsupported(self.at, Form::Projection))?;
        let at = self.at_offset(addr, offset);
        let found = self.b.load(int, at, Access::Plain, self.at);
        let pattern = self.b.constant(int, pattern, self.at);
        Ok(self.b.compare(cond, found, pattern, self.at))
    }

    /// The position of the variant the value of type `ty` at `addr` is, a word: 0 for a struct.
    pub(crate) fn variant_index(&mut self, addr: Value, ty: Ty) -> Result<Value, Unlowered> {
        let unsupported = || Unlowered::unsupported(self.at, Form::Projection);
        let shape = self.l.types.shape(ty, self.at)?;
        let word = self.l.types.word;
        // A struct is its one variant.
        let Parts::Enum { tag, .. } = &shape.parts else { return Ok(self.b.constant(word, 0, self.at)) };
        let (offset, bytes, niche) = match tag {
            Tag::Alone => return Ok(self.b.constant(word, 0, self.at)),
            Tag::Index { bytes, .. } => (0, *bytes, None),
            Tag::Niche { empty, offset, bytes, value } => (*offset, *bytes, Some((*empty, *value))),
        };
        let int = Scalar::int(u32::try_from(bytes * 8).unwrap_or_default()).ok_or_else(unsupported)?;
        let at = self.at_offset(addr, offset);
        let found = self.b.load(int, at, Access::Plain, self.at);
        // Tags that name themselves are told apart one by one, the last variant being what no
        // other pattern is.
        if let Tag::Index { codes: Some(codes), .. } = tag {
            let last = codes.len().saturating_sub(1);
            let mut index = self.b.constant(word, last as u128, self.at);
            for (i, &pattern) in codes.iter().enumerate().take(last).rev() {
                let pattern = self.b.constant(int, pattern, self.at);
                let is = self.b.compare(Cond::Eq, found, pattern, self.at);
                let at = self.b.constant(word, i as u128, self.at);
                index = self.b.select(is, at, index, self.at);
            }
            return Ok(index);
        }
        let Some((empty, pattern)) = niche else { return Ok(self.resize(found, int, false, word)) };
        // Of the two variants a niche tells apart, the empty one holds the pattern.
        let pattern = self.b.constant(int, pattern, self.at);
        let is_empty = self.b.compare(Cond::Eq, found, pattern, self.at);
        let other = self.b.constant(word, u128::from(empty == 0), self.at);
        let empty = self.b.constant(word, empty as u128, self.at);
        Ok(self.b.select(is_empty, empty, other, self.at))
    }

    /// Whether the value of type `ty` at `addr` is its variant `variant`, `None` for the one
    /// variant a struct is, and the address of the part `index` of that variant.
    pub(crate) fn part_at(
        &mut self,
        addr: Value,
        ty: Ty,
        variant: usize,
        index: usize,
    ) -> Result<(Option<Value>, Value), Unlowered> {
        let unsupported = || Unlowered::unsupported(self.at, Form::Projection);
        let shape = self.l.types.shape(ty, self.at)?;
        match &shape.parts {
            Parts::Sequence(offsets) if variant == 0 => {
                let offset = *offsets.get(index).ok_or_else(unsupported)?;
                Ok((None, self.at_offset(addr, offset)))
            }
            Parts::Enum { .. } => {
                let members = self.l.types.cx.get::<Members>(&ty).ok_or_else(unsupported)?;
                let name = members.variants.get(variant).ok_or_else(unsupported)?.name;
                let holds = self.is_variant(addr, ty, name)?;
                Ok((Some(holds), self.payload(addr, ty, name, index)?))
            }
            _ => Err(unsupported()),
        }
    }

    /// Makes the enum of type `ty` at `addr` the variant `variant`, its payload written apart.
    pub(crate) fn set_variant(&mut self, addr: Value, ty: Ty, variant: Symbol) -> Result<(), Unlowered> {
        let (index, shape) = self.variant(ty, variant)?;
        let Parts::Enum { tag, .. } = &shape.parts else {
            return Err(Unlowered::unsupported(self.at, Form::Projection));
        };
        let (offset, bytes, pattern) = match tag {
            Tag::Index { bytes, .. } => {
                let pattern = tag.pattern(index).ok_or(Unlowered::unsupported(self.at, Form::Projection))?;
                (0, *bytes, pattern)
            }
            Tag::Niche { empty, offset, bytes, value } if *empty == index => (*offset, *bytes, *value),
            Tag::Alone | Tag::Niche { .. } => return Ok(()),
        };
        let int = Scalar::int(u32::try_from(bytes * 8).unwrap_or_default())
            .ok_or(Unlowered::unsupported(self.at, Form::Projection))?;
        let at = self.at_offset(addr, offset);
        let pattern = self.b.constant(int, pattern, self.at);
        self.b.store(pattern, at, Access::Plain, self.at);
        Ok(())
    }
}
