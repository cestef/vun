//! The `sarifLog` of a run: the results reported so far, and the rules they name.

use vun_data::{Object, Value};
use vun_diag::{DiagId, Diagnostic};
use vun_origin::Resolve;
use vun_span::SourceMap;

use crate::place::Places;

/// The version of the format written.
pub const VERSION: &str = "2.1.0";

/// The schema of that version, which editors and services validate a log against.
pub const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

/// The tool that ran.
const TOOL: &str = "vun";

/// The results of a run, in the order they were reported.
#[derive(Default)]
pub struct Log {
    results: Vec<(DiagId, Object<'static>)>,
}

impl Log {
    /// Adds the result of `d`, its places resolved now.
    pub fn add(&mut self, d: &Diagnostic, origins: &dyn Resolve, sources: &dyn SourceMap) {
        let result = Places { origins, sources }.result(d);
        self.results.push((d.id, result));
    }

    /// The log: one run, whose rules are the ids of the results in the order each first
    /// appears, each result naming its rule by index.
    #[must_use]
    pub fn document(&self) -> Value<'static> {
        let mut rules: Vec<&DiagId> = Vec::new();
        let results: Vec<_> = self
            .results
            .iter()
            .map(|(id, result)| {
                let index = rules.iter().position(|r| *r == id).unwrap_or_else(|| {
                    rules.push(id);
                    rules.len() - 1
                });
                result.clone().with("ruleIndex", index)
            })
            .collect();
        let rules: Vec<_> = rules
            .into_iter()
            .map(|id| Object::new().with("id", id.to_string()).with("name", id.path().to_string()))
            .collect();
        let driver =
            Object::new().with("name", TOOL).with("version", env!("CARGO_PKG_VERSION")).with("rules", rules);
        let run = Object::new().with("tool", Object::new().with("driver", driver)).with("results", results);
        Value::Object(Object::new().with("$schema", SCHEMA).with("version", VERSION).with("runs", vec![run]))
    }
}
