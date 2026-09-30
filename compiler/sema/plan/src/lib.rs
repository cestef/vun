//! A package's plan read as steps (spec 12, section 2): `vun_sema::plan::Planned` runs a
//! `build.vun`'s `build`, and each value it records is read here as what it does.

mod action;

pub use action::{Action, Options, Placement, Region, Use};
pub use vun_sema::plan::{Plan, Planned, Unplanned};
