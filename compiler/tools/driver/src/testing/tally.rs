//! The line closing a run of tests: how many passed, trapped, were falsified, were not run, were
//! measured and were fuzzed.

use vun_data::{Object, ToData};
use vun_term::{Nanos, Noun, Record, Status, Style, Text, Verb};

use super::Totals;

const PASSED: Noun = Noun { one: "passed", many: "passed" };
const TRAPPED: Noun = Noun { one: "trapped", many: "trapped" };
const FALSIFIED: Noun = Noun { one: "falsified", many: "falsified" };
const IGNORED: Noun = Noun { one: "ignored", many: "ignored" };
const MEASURED: Noun = Noun { one: "measured", many: "measured" };
const FUZZED: Noun = Noun { one: "fuzzed", many: "fuzzed" };

/// How many tests passed, trapped, were falsified, were not run, were measured and were fuzzed.
pub(super) struct Tally(pub(super) Totals);

impl Record for Tally {
    fn kind(&self) -> &'static str {
        "tested"
    }

    fn text(&self) -> Text {
        let Totals { passed, trapped, falsified, ignored, measured, fuzzed, nanos } = self.0;
        // What did not happen is left out, except the passes: a run that passed nothing says so.
        let counts = [
            (PASSED, passed, true),
            (TRAPPED, trapped, false),
            (FALSIFIED, falsified, false),
            (IGNORED, ignored, false),
            (MEASURED, measured, false),
            (FUZZED, fuzzed, false),
        ];
        let counts = counts.into_iter().filter(|&(_, n, always)| always || n > 0);
        let counts = Text::join(counts.map(|(noun, n, _)| Text::from(noun.count(n).to_string())), ", ");
        match nanos {
            Some(nanos) => counts.append(Text::of(Style::Dim, format!(" in {}", Nanos::from(nanos)))),
            None => counts,
        }
    }

    fn fields(&self) -> Object<'static> {
        let Totals { passed, trapped, falsified, ignored, measured, fuzzed, nanos } = self.0;
        let object = Object::new().with("passed", passed).with("trapped", trapped);
        let object = object.with("falsified", falsified).with("ignored", ignored);
        object.with("measured", measured).with("fuzzed", fuzzed).with("nanos", nanos.to_data())
    }
}

impl Status for Tally {
    fn verb(&self) -> Verb {
        if self.0.trapped == 0 && self.0.falsified == 0 {
            Verb::good("Tested")
        } else {
            Verb::failed("Tested")
        }
    }
}
