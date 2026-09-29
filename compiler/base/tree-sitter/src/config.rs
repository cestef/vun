//! `tree-sitter.json`, which says what a grammar is named and which files it reads.

use vun_data::ToData;

/// The configuration of a grammar repository holding one grammar.
#[derive(Clone, Debug, PartialEq, Eq, ToData)]
pub struct Config {
    pub grammars: Vec<GrammarConfig>,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData)]
pub struct GrammarConfig {
    pub name: String,
    /// The scope editors name its files by: `source.vun`.
    pub scope: String,
    /// The directory of `grammar.js`, relative to the configuration.
    pub path: String,
    #[data(name = "file-types")]
    pub file_types: Vec<String>,
    /// The highlight queries, relative to the configuration.
    pub highlights: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData)]
pub struct Metadata {
    pub version: String,
}

impl Config {
    /// The configuration of the grammar `name` for files ending in `extension`, its highlights
    /// at `highlights`.
    #[must_use]
    pub fn of(name: &str, extension: &str, highlights: &str, version: &str) -> Self {
        let grammar = GrammarConfig {
            name: name.to_owned(),
            scope: ["source", name].join("."),
            path: ".".to_owned(),
            file_types: vec![extension.to_owned()],
            highlights: vec![highlights.to_owned()],
        };
        Self { grammars: vec![grammar], metadata: Metadata { version: version.to_owned() } }
    }
}
