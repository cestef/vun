//! A whole grammar, written as `grammar.js`.

use std::fmt;

use vun_data::Value;
use vun_data::json::Json;

use crate::Rule;

/// A tree-sitter grammar: its rules, the first its root.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grammar {
    pub name: String,
    /// The token keywords are extracted from, by rule name.
    pub word: Option<String>,
    /// What may stand between any two tokens.
    pub extras: Vec<Rule>,
    /// The tokens an external scanner reads.
    pub externals: Vec<Rule>,
    /// Sets of rules the parser keeps apart where they conflict, by name.
    pub conflicts: Vec<Vec<String>>,
    /// A JSON file beside `grammar.js` holding more such sets, as a list of lists of names,
    /// which a tool finds and writes.
    pub conflicts_file: Option<String>,
    pub rules: Vec<(String, Rule)>,
}

/// `[a, b]`.
fn list(f: &mut fmt::Formatter<'_>, items: impl IntoIterator<Item = impl fmt::Display>) -> fmt::Result {
    let items: Vec<String> = items.into_iter().map(|i| i.to_string()).collect();
    write!(f, "[{}]", items.join(", "))
}

impl fmt::Display for Grammar {
    /// The `grammar.js` of this grammar.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "module.exports = grammar({{")?;
        writeln!(f, "  name: {},", Json(&Value::from(self.name.clone())))?;
        if let Some(word) = &self.word {
            writeln!(f, "  word: $ => {},", Rule::symbol(word))?;
        }
        for (name, rules) in [("extras", &self.extras), ("externals", &self.externals)] {
            if !rules.is_empty() {
                write!(f, "  {name}: $ => ")?;
                list(f, rules)?;
                writeln!(f, ",")?;
            }
        }
        let mut sets: Vec<String> = self
            .conflicts
            .iter()
            .map(|set| {
                let rules: Vec<String> = set.iter().map(|r| Rule::symbol(r).to_string()).collect();
                format!("[{}]", rules.join(", "))
            })
            .collect();
        if let Some(file) = &self.conflicts_file {
            let path = Json(&Value::from(["./", file].concat())).to_string();
            sets.push(format!("...require({path}).map((set) => set.map((name) => $[name]))"));
        }
        if !sets.is_empty() {
            write!(f, "  conflicts: $ => ")?;
            list(f, sets)?;
            writeln!(f, ",")?;
        }
        writeln!(f, "  rules: {{")?;
        for (name, rule) in &self.rules {
            writeln!(f, "    {name}: $ => {rule},")?;
        }
        writeln!(f, "  }},")?;
        writeln!(f, "}});")
    }
}
