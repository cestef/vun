//! Moves out of borrows: a linear value read whole from a place whose path crosses the `.*` of
//! a borrow. Crossing an owner's `.*` is not one, for an owner held by value owns its target.

use vun_mir::{Body, Local, Operand, Place, Projection, Rvalue};
use vun_origin::OriginId;
use vun_query::Cx;
use vun_sema::kinds::Kinds;

/// The reads moving a linear value out of a borrow: where, and the linear local it lands in when
/// it is copied into one whole.
pub(super) struct Moved;

impl Moved {
    pub(super) fn find(
        cx: &Cx<'_>,
        body: &Body,
        linear: impl Fn(Local) -> bool,
    ) -> Vec<(Option<Local>, OriginId)> {
        let moves = |place: &Place| {
            Self::crosses_borrow(cx, body, place)
                && place.ty(cx, &body.locals).is_some_and(|t| cx.get::<Kinds>(&t).is_linear())
        };
        let mut found = Vec::new();
        for (_, block) in body.blocks.iter() {
            for stmt in &block.stmts {
                let landing = match &stmt.value {
                    Rvalue::Use(_) if stmt.place.projection.is_empty() && linear(stmt.place.local) => {
                        Some(stmt.place.local)
                    }
                    _ => None,
                };
                let sources = stmt.value.operands().into_iter().filter_map(Self::place);
                found.extend(sources.filter(|&p| moves(p)).map(|_| (landing, stmt.origin)));
            }
            let sources = block.term.kind.operands().into_iter().filter_map(Self::place);
            found.extend(sources.filter(|&p| moves(p)).map(|_| (None, block.term.origin)));
        }
        found
    }

    fn place(operand: &Operand) -> Option<&Place> {
        match operand {
            Operand::Place(place) => Some(place),
            Operand::Const(_) => None,
        }
    }

    /// Whether the path of `place` goes through a borrow, by the types along it; not when a type
    /// on the way is unknown.
    fn crosses_borrow(cx: &Cx<'_>, body: &Body, place: &Place) -> bool {
        let types = place.types(cx, &body.locals);
        place
            .projection
            .iter()
            .zip(&types)
            .any(|(p, ty)| *p == Projection::Deref && ty.is_some_and(|t| t.borrowed().is_some()))
    }
}
