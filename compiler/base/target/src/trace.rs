//! How much of a program's code its table describes (spec 07, section 3).

use vun_codec::{Decode, Encode};

/// `lang.trace.Detail`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum Detail {
    /// No table: a trap reports nothing.
    Off,
    /// The trap sites: where a trap is written, and what it says.
    Sites,
    /// The sites, and the functions of the calls that led to a trap.
    #[default]
    Calls,
}
