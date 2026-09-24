//! The types of places, followed along their projections from the local's type; `None` where a
//! type on the way is not known.

use vun_index::IndexVec;
use vun_query::Cx;
use vun_sema::member::Members;
use vun_sema::ty::{Ty, TyKind, Value};

use crate::{Local, LocalDecl, Place, Projection};

impl Place {
    /// The type of each prefix of this place among `locals`: the local's, then after each
    /// projection.
    #[must_use]
    pub fn types(&self, cx: &Cx<'_>, locals: &IndexVec<Local, LocalDecl>) -> Vec<Option<Ty>> {
        let mut types = vec![locals[self.local].ty];
        for projection in &self.projection {
            let next = types.last().copied().flatten().and_then(|ty| Self::step(cx, locals, ty, projection));
            types.push(next);
        }
        types
    }

    /// The type of this place among `locals`, where known.
    #[must_use]
    pub fn ty(&self, cx: &Cx<'_>, locals: &IndexVec<Local, LocalDecl>) -> Option<Ty> {
        self.types(cx, locals).pop().flatten()
    }

    /// The type after one projection of a value of type `ty`.
    fn step(cx: &Cx<'_>, locals: &IndexVec<Local, LocalDecl>, ty: Ty, projection: &Projection) -> Option<Ty> {
        match projection {
            Projection::Deref => ty.pointee(),
            Projection::Field(name) => {
                if let TyKind::Record(fields) = &*ty {
                    return fields.iter().find(|(n, _)| n == name).map(|(_, t)| *t);
                }
                let members = cx.get::<Members>(&ty)?;
                members.fields.iter().find(|f| f.name == *name).and_then(|f| f.ty.as_ref().ok().copied())
            }
            Projection::Position(at) => match &*ty {
                TyKind::Tuple(items) => items.get(*at).copied(),
                _ => None,
            },
            Projection::Payload { variant, at } => {
                let members = cx.get::<Members>(&ty)?;
                let variant = members.variants.iter().find(|v| v.name == *variant)?;
                variant.payload.get(*at)?.as_ref().ok().copied()
            }
            // Items at bounds are a run of the sequence, as its slice type.
            Projection::Index(index) if locals[*index].ty.is_some_and(|t| t.bounds().is_some()) => {
                ty.items().map(TyKind::slice)
            }
            Projection::Index(_) => match &*ty {
                TyKind::Primitive(_, args) => args.iter().rev().find_map(Value::as_type),
                _ => None,
            },
        }
    }
}
