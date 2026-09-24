//! The types of places, followed along their projections from the local's type; `None` where a
//! type on the way is not known.

use overt_mir::{Body, Place, Projection};
use overt_query::Cx;
use overt_sema::member::Members;
use overt_sema::ty::{Ty, TyKind, Value};

/// Place types in one body.
pub(crate) struct PlaceTypes<'c, 'b> {
    pub(crate) cx: &'c Cx<'c>,
    pub(crate) body: &'b Body,
}

impl PlaceTypes<'_, '_> {
    /// The type of each prefix of `place`: the local's, then after each projection.
    pub(crate) fn along(&self, place: &Place) -> Vec<Option<Ty>> {
        let mut types = vec![self.body.locals[place.local].ty];
        for projection in &place.projection {
            let next = types.last().copied().flatten().and_then(|ty| self.step(ty, projection));
            types.push(next);
        }
        types
    }

    /// The type after one projection of a value of type `ty`.
    fn step(&self, ty: Ty, projection: &Projection) -> Option<Ty> {
        match projection {
            Projection::Deref => ty.pointee(),
            Projection::Field(name) => {
                if let TyKind::Record(fields) = &*ty {
                    return fields.iter().find(|(n, _)| n == name).map(|(_, t)| *t);
                }
                let members = self.cx.get::<Members>(&ty)?;
                members.fields.iter().find(|f| f.name == *name).and_then(|f| f.ty.as_ref().ok().copied())
            }
            Projection::Position(at) => match &*ty {
                TyKind::Tuple(items) => items.get(*at).copied(),
                _ => None,
            },
            Projection::Payload(at) => {
                // The type of an item of a payload: known when every variant with one agrees.
                let members = self.cx.get::<Members>(&ty)?;
                let mut items =
                    members.variants.iter().filter_map(|v| v.payload.get(*at)?.as_ref().ok().copied());
                let first = items.next()?;
                items.all(|t| t == first).then_some(first)
            }
            Projection::Index(_) => match &*ty {
                TyKind::Primitive(_, args) => args.iter().rev().find_map(Value::as_type),
                _ => None,
            },
        }
    }
}
