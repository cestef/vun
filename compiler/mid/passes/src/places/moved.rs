//! Moves out of borrows: a linear value read whole from a place whose path crosses the `.*` of
//! a borrow. Crossing an owner's `.*` is not one, for an owner held by value owns its target.

use overt_mir::{Body, Local, Operand, Place, Projection, Rvalue};
use overt_origin::OriginId;
use overt_query::Cx;

use crate::types::PlaceTypes;

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
        let types = PlaceTypes { cx, body }.along(place);
        place
            .projection
            .iter()
            .zip(&types)
            .any(|(p, ty)| *p == Projection::Deref && ty.is_some_and(|t| t.borrowed().is_some()))
    }
}
