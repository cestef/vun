//! MIR checks and transforms.

pub mod borrows;
pub mod coroutines;
pub mod kinds;
pub mod panics;
pub mod places;
pub mod ranges;

use vun_query::Db;

/// The checks on MIR whose memos outlive the process, with their values.
pub const PERSISTED: &[fn(&Db)] = &[
    Db::persist::<borrows::BorrowChecks>,
    Db::persist::<panics::PanicChecks>,
    Db::persist::<places::Placed>,
    Db::persist::<places::PlaceStates>,
];
