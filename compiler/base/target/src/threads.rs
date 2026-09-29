//! How many threads the programs of a target run (spec 07, section 2).

use vun_codec::{Decode, Encode};

/// `lang.target.Threads`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Threads {
    /// One: storage per thread is the program's.
    #[default]
    One,
    /// As many as they start, each with storage of its own.
    Many,
}
