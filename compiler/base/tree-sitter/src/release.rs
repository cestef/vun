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

    /// What the grammar's repository holds of the grammar's directory, each a file or a directory
    /// copied whole: the grammar, its conflicts, its configuration, its parser and its queries.
    pub const PUBLISHED: &'static [&'static str] =
        &["grammar.js", "conflicts.json", "tree-sitter.json", "src", "queries"];
}
