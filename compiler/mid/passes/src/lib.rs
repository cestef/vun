//! MIR checks and transforms.

pub mod borrows;
pub mod coroutines;
mod flow;
pub mod kinds;
pub mod panics;
pub mod places;
pub mod ranges;
pub mod widths;

use vun_query::Db;

/// The checks on MIR whose memos outlive the process, with their values; what a call may do is
/// judged again where it is read (`MayTrap` answers a cycle with a judgement of its own), its
/// reads and fingerprints kept so that the checks reading it persist.
pub const PERSISTED: &[fn(&Db)] = &[
    Db::persist::<borrows::BorrowChecks>,
    Db::persist::<panics::PanicChecks>,
    Db::persist::<places::Placed>,
    Db::persist::<places::PlaceStates>,
    Db::persist::<widths::WideChecks>,
    Db::trace::<panics::MayTrap>,
];
