//! A pattern's type through where typing gave up.

use vun_hir::tree::ExprId;
use vun_sema_ty::Ty;

use super::{GaveUp, GaveUps, Known, Typer};

impl Typer<'_, '_, '_> {
    /// The type of `value`, typed as `known`, that a pattern matches, or why it has none: why
    /// typing gave up under it, or the error typing reported there.
    pub(super) fn matched(&self, value: ExprId, known: Option<Known>) -> Result<Ty, GaveUp> {
        let why =
            || self.found.first_under(self.hir(), value).map_or(GaveUp::Reported, |(_, why)| why.clone());
        known.map(|k| k.ty).ok_or_else(why)
    }
}
