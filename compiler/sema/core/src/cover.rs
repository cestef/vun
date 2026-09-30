//! Whether the programs built mark the blocks they run (spec 11, section 6): an input the driver
//! sets, as it sets the checks, for the programs `vun fuzz` builds; the MIR of each function then
//! sets the flag of each of its blocks in `lang.cover.map` as the block starts.

use vun_codec::{Decode, Encode};
use vun_query::{Cx, Query};

/// What the programs built record of what they run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Coverage {
    /// Nothing.
    #[default]
    Off,
    /// A flag per block, set as it starts.
    Blocks,
}

/// The coverage of the compilation: none until the driver sets it.
pub struct CoverageOf;

impl Query for CoverageOf {
    const NAME: &'static str = "coverage";
    const INPUT: bool = true;
    type Key = ();
    type Value = Coverage;

    fn compute(_: &Cx<'_>, (): &()) -> Self::Value {
        Coverage::Off
    }
}
