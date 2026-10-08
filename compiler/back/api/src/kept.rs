//! Machine code kept between builds (spec 13, section 2; decision 669): the code an earlier build
//! compiled for a function of the unit, which a backend takes instead of compiling it again.

use vun_lir::FuncId;

use crate::Compiled;

/// The code kept for the functions of a unit, its relocations in the unit's numbering, and where
/// the code compiled anew is kept.
pub trait Kept: Sync {
    /// The code kept for `id`. The driver leaves such a function as lowered (`Legalize::apply`),
    /// for only a backend keeping code ([`Feature::Keeps`](crate::Feature::Keeps)) is given it.
    fn code(&self, id: FuncId) -> Option<Compiled<()>>;

    /// Keeps `code`, compiled anew for `id`.
    fn keep(&self, id: FuncId, code: Compiled<()>);
}
