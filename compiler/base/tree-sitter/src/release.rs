//! What a grammar's repository last published: the revision editors build, recorded beside the
//! grammar by whoever publishes it, and read by what generates the editors' packages.

use vun_data::{FromData, ToData};

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Release {
    /// The commit of the grammar's repository holding the grammar and its parser.
    pub revision: String,
}

impl Release {
    /// Its file, beside `grammar.js`.
    pub const FILE: &'static str = "release.json";
}
