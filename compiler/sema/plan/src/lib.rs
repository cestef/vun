//! A package's plan read as steps (spec 12, section 2): `Planned` runs a
//! `build.vun`'s `build`, and each value it records is read here as what it does.

mod action;
mod method;
mod planned;

pub use action::{Action, Options, Placement, Region, Use};
pub use method::{Argument, Delivery, Invocation, Method};
pub use planned::{Plan, Planned, Unplanned};

/// The queries of this crate whose memos persist.
pub const PERSISTED: &[fn(&vun_query::Db)] = &[vun_query::Db::persist::<Planned>];
