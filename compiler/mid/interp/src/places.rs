//! Places at compile time: a local and the parts under it. `.*` of a shared borrow is the value
//! itself, since a shared borrow of a compile-time value reads as the value; `.*` of a loan, an
//! exclusive borrow, goes on at the place it reaches.

use vun_index::Idx;
use vun_mir::{Local, Place, Projection, Role};
use vun_sema::decl::DeclValue;
use vun_sema::env::Unevaluated;
use vun_sema::ops::Part;
use vun_sema::ty::Value;

use crate::machine::Frame;

impl Frame<'_, '_, '_> {
    /// What `local` holds; a `mut` static's is known at run time only.
    pub(crate) fn local(&self, local: Local) -> Result<Value, Unevaluated> {
        match &self.body.locals[local].role {
            Role::StaticMut(_) => Err(Unevaluated::Runtime),
            // A top-level binding's storage holds its value, which never changes.
            Role::Static(decl) => {
                self.locals[local].clone().map_or_else(|| self.env.cx.get::<DeclValue>(decl), Ok)
            }
            _ => self.locals[local].clone().ok_or(Unevaluated::Unsupported),
        }
    }

    /// The local and parts `place` reaches, through the loans its `.*` cross.
    pub(crate) fn resolve(&self, place: &Place) -> Result<(Local, Vec<Part>), Unevaluated> {
        let (mut local, mut parts) = (place.local, Vec::new());
        for projection in &place.projection {
            match self.part(projection)? {
                Some(part) => parts.push(part),
                None => {
                    if let Value::Loan { local: lent, parts: under } = self.at(local, &parts)? {
                        local = self.lent(lent)?;
                        parts = under;
                    }
                }
            }
        }
        Ok((local, parts))
    }

    /// The local a loan of this call names.
    pub(crate) fn lent(&self, local: usize) -> Result<Local, Unevaluated> {
        (local < self.locals.len()).then(|| Local::new(local)).ok_or(Unevaluated::Unsupported)
    }

    /// The value at `parts` under `local`.
    pub(crate) fn at(&self, local: Local, parts: &[Part]) -> Result<Value, Unevaluated> {
        parts.iter().try_fold(self.local(local)?, |value, part| value.part(part))
    }

    /// The value at `place`.
    pub(crate) fn read(&self, place: &Place) -> Result<Value, Unevaluated> {
        let (local, parts) = self.resolve(place)?;
        match self.at(local, &parts)? {
            // A lent parameter read whole is lent on: a loan of it, as its caller's loan is.
            value
                if place.projection.is_empty()
                    && self.exclusive(local)
                    && !matches!(value, Value::Loan { .. }) =>
            {
                Ok(Value::Loan { local: local.index(), parts })
            }
            value => Ok(value),
        }
    }

    /// Stores `value` at `place`: a whole local, or a part of one, rebuilt around it.
    pub(crate) fn write(&mut self, place: &Place, value: Value) -> Result<(), Unevaluated> {
        let (local, parts) = self.resolve(place)?;
        self.store(local, &parts, value)
    }

    /// Stores `value` at `parts` under `local`.
    pub(crate) fn store(&mut self, local: Local, parts: &[Part], value: Value) -> Result<(), Unevaluated> {
        let value = if parts.is_empty() { value } else { Self::replaced(self.local(local)?, parts, value)? };
        self.locals[local] = Some(value);
        Ok(())
    }

    /// `whole` with the part `parts` reach replaced by `value`.
    fn replaced(whole: Value, parts: &[Part], value: Value) -> Result<Value, Unevaluated> {
        let Some((first, rest)) = parts.split_first() else { return Ok(value) };
        let inner = Self::replaced(whole.part(first)?, rest, value)?;
        whole.with_part(first, inner)
    }

    /// The part of a value `projection` reaches, its index read; `None` for `.*`.
    pub(crate) fn part(&self, projection: &Projection) -> Result<Option<Part>, Unevaluated> {
        Ok(Some(match projection {
            Projection::Deref => return Ok(None),
            Projection::Field(name) => Part::Field(*name),
            Projection::Index(index) => Part::Item(self.local(*index)?),
            Projection::Position(at) => Part::Position(*at),
            Projection::Payload { at, .. } => Part::Payload(*at),
        }))
    }
}
