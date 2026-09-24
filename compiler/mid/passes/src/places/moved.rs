//! Moves out of borrows: a linear value read whole from a place whose path crosses the `.*` of
//! a borrow. Crossing an owner's `.*` is not one, for an owner held by value owns its target.

use overt_mir::{Body, Local, Operand, Place, Projection, Rvalue};
use overt_origin::OriginId;
use overt_query::Cx;
use overt_sema::member::Members;
use overt_sema::ty::{Ty, TyKind, Value};

/// The statements moving a linear value out of a borrow: the local it lands in, and where.
pub(super) struct Moved;

impl Moved {
    pub(super) fn find(cx: &Cx<'_>, body: &Body, linear: impl Fn(Local) -> bool) -> Vec<(Local, OriginId)> {
        let mut moved = Vec::new();
        for (_, block) in body.blocks.iter() {
            for stmt in &block.stmts {
                if let Rvalue::Use(Operand::Place(source)) = &stmt.value
                    && stmt.place.projection.is_empty()
                    && linear(stmt.place.local)
                    && Self::crosses_borrow(cx, body, source)
                {
                    moved.push((stmt.place.local, stmt.origin));
                }
            }
        }
        moved
    }

    /// Whether the path of `place` goes through a borrow, by the types along it; not when a type
    /// on the way is unknown.
    fn crosses_borrow(cx: &Cx<'_>, body: &Body, place: &Place) -> bool {
        let Some(mut ty) = body.locals[place.local].ty else { return false };
        for projection in &place.projection {
            let next = match projection {
                Projection::Deref if ty.borrowed().is_some() => return true,
                Projection::Deref => ty.pointee(),
                Projection::Field(name) => cx.get::<Members>(&ty).and_then(|m| {
                    m.fields.iter().find(|f| f.name == *name).and_then(|f| f.ty.as_ref().ok().copied())
                }),
                Projection::Position(at) => match &*ty {
                    TyKind::Tuple(items) => items.get(*at).copied(),
                    _ => None,
                },
                Projection::Payload(at) => Self::payload(cx, ty, *at),
                Projection::Index(_) => match &*ty {
                    TyKind::Primitive(_, args) => args.iter().rev().find_map(Value::as_type),
                    _ => None,
                },
            };
            let Some(next) = next else { return false };
            ty = next;
        }
        false
    }

    /// The type of an item of a payload of `ty`'s variants: known when every variant with one
    /// that long agrees.
    fn payload(cx: &Cx<'_>, ty: Ty, at: usize) -> Option<Ty> {
        let members = cx.get::<Members>(&ty)?;
        let mut items = members.variants.iter().filter_map(|v| v.payload.get(at)?.as_ref().ok().copied());
        let first = items.next()?;
        items.all(|t| t == first).then_some(first)
    }
}
