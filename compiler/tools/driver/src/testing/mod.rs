//! Running tests (spec 11): the program `std.testing.runner` gives for a namespace, built from a
//! root file as any program is, and the lines it writes, read as typed values and reported.

mod build;
mod report;

pub use report::{Reported, Run};

use vun_command::{Args, Param, Values};
use vun_data::{FromData, ToData};

pub const SEED: Param = Param::option("seed")
    .about("draw the cases of every property from this seed, as a falsified one reports it")
    .values(Values::Count);
pub const CASES: Param = Param::option("cases")
    .about("how many cases of each property to run; 100 if not given")
    .values(Values::Count);

/// How a run of the tests' program is asked for besides the tests it names: the options
/// `std.testing.Options` reads from the program's arguments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub seed: Option<u64>,
    pub cases: Option<u64>,
}

impl Options {
    /// The options a command line gives.
    #[must_use]
    pub fn of(args: &Args) -> Self {
        Self { seed: args.count(&SEED), cases: args.count(&CASES) }
    }

    /// Its arguments to the tests' program: each option given, as written, and its number.
    fn args(self) -> impl Iterator<Item = String> {
        let given = [(SEED, self.seed), (CASES, self.cases)].into_iter();
        given.filter_map(|(param, n)| Some([param.long_form(), n?.to_string()])).flatten()
    }
}

/// A line the runner writes (`std.testing.Line`): a test run, then the totals.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub enum Line {
    Test(Ran),
    Totals(Totals),
}

/// A test run: its name, as declared, and what became of it.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Ran {
    pub name: String,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub enum Outcome {
    Passed,
    Ignored(Ignored),
    Trapped(Trapped),
    Falsified(Falsified),
}

impl Outcome {
    /// The trap that ended the test, or the smallest failing case of a property.
    #[must_use]
    pub fn trap(&self) -> Option<&Trapped> {
        match self {
            Self::Trapped(trapped) | Self::Falsified(Falsified { trapped, .. }) => Some(trapped),
            Self::Passed | Self::Ignored(_) => None,
        }
    }
}

/// A property test's failing case, shrunk (`std.testing.Falsified`): the trap of the smallest
/// case found, the seed its cases were drawn from, the case that trapped first and how many
/// smaller cases shrinking found.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Falsified {
    pub trapped: Trapped,
    pub seed: u64,
    pub case: usize,
    pub shrinks: usize,
}

/// A test that is not run, and why (`std.testing.Ignored`).
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Ignored {
    pub reason: String,
}

/// A trap ending a test: its site, and what the program's table says of it.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Trapped {
    pub site: u32,
    pub said: Option<Said>,
}

/// What the program's table says of a trap site (`std.trace.Site`).
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Said {
    pub message: Option<String>,
    pub at: Option<Place>,
}

/// A place in source, its line and column counted from 1 (`std.trace.Place`).
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Place {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// How many tests passed, trapped, were falsified and were not run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Totals {
    pub passed: usize,
    pub trapped: usize,
    pub falsified: usize,
    pub ignored: usize,
}

impl Line {
    /// The line the runner wrote as `text`; `None` for what the tests write themselves.
    #[must_use]
    pub fn of(text: &str) -> Option<Self> {
        vun_data::json::parse(text).ok()?.read().ok()
    }
}
