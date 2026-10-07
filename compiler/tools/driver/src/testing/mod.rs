//! Running tests (spec 11): the program `std.testing.runner` gives for a namespace, built from a
//! root file as any program is, and the lines it writes, read as typed values and reported.

mod bench;
mod build;
mod parallel;
mod report;
mod tally;

pub use bench::{
    BASELINE, Baseline, Comparison, FAIL_OVER, MEASURE, Measure, Plan, SAMPLES, SAVE, Statistics, Verdict,
};
pub use report::{Collected, Reported, Run, Tested};

use vun_command::{Args, Param, Values};
use vun_data::{FromData, ToData};

pub const JOBS: Param = Param::option("jobs")
    .short('j')
    .about("how many tests run at once, each in a process of its own; the machine's cores if not given, 1 runs them in one process, in order")
    .values(Values::Count)
    .env("VUN_TEST_JOBS");
pub const SEED: Param = Param::option("seed")
    .about("draw the cases of every property from this seed, as a falsified one reports it")
    .values(Values::Count);
pub const CASES: Param = Param::option("cases")
    .about("how many cases of each property to run; 100 if not given")
    .values(Values::Count);
/// `--cases` as `vun fuzz` reads it, fuzzing longer.
pub const FUZZED: Param = Param::option("cases")
    .about("how many cases of each property to fuzz; 10000 if not given")
    .values(Values::Count);
/// The runner's options asking for the benchmarks alone, measured, and for the properties alone,
/// fuzzed: what `vun bench` and `vun fuzz` pass.
const BENCH: Param = Param::flag("bench");
const FUZZ: Param = Param::flag("fuzz");
/// The runner's option writing the coverage flags the tests set.
const COVER: Param = Param::flag("cover");
/// The runner's option writing the tests it would run, and running none.
const LIST: Param = Param::flag("list");

/// How many cases `vun fuzz` runs of each property unless `--cases` asks for another count.
const FUZZED_CASES: u64 = 10_000;

/// What a run of the tests' program does with them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Runs every test.
    #[default]
    Test,
    /// Measures the benchmarks alone.
    Bench,
    /// Fuzzes the properties alone, in a program built covered.
    Fuzz,
    /// Runs every test in a program built covered, which writes the flags they set.
    Cover,
}

/// How a run of the tests' program is asked for besides the tests it names: the options
/// `std.testing.Options` reads from the program's arguments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub seed: Option<u64>,
    pub cases: Option<u64>,
    pub mode: Mode,
}

impl Options {
    /// The options a command line gives for a run in `mode`.
    #[must_use]
    pub fn of(args: &Args, mode: Mode) -> Self {
        let cases = args.count(&CASES).or((mode == Mode::Fuzz).then_some(FUZZED_CASES));
        Self { seed: args.count(&SEED), cases, mode }
    }

    /// Its arguments to the tests' program: each option given, as written, and its number, then
    /// the mode's flag.
    fn args(self) -> impl Iterator<Item = String> {
        let given = [(SEED, self.seed), (CASES, self.cases)].into_iter();
        let numbers = given.filter_map(|(param, n)| Some([param.long_form(), n?.to_string()])).flatten();
        let flag = match self.mode {
            Mode::Test => None,
            Mode::Bench => Some(BENCH),
            Mode::Fuzz => Some(FUZZ),
            Mode::Cover => Some(COVER),
        };
        numbers.chain(flag.map(|flag| flag.long_form()))
    }
}

/// A line the runner writes (`std.testing.Line`): a test run, then the totals; before a falsified
/// property's line, each argument of its failing case (`std.testing.Argued`).
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub enum Line {
    Test(Ran),
    Totals(Totals),
    Argument(Argument),
    /// A test the runner lists when asked to list (`std.testing.Listed`).
    Listed(Listed),
    /// The coverage counts the tests make, before the totals when asked to cover.
    Covered(Covered),
}

/// The counts of `lang.cover.map` the tests make (`std.testing.Covered`): how many times a block
/// of that number started.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Covered {
    pub counts: Vec<u32>,
}

impl Covered {
    /// How many times a block of the number `flag` started.
    #[must_use]
    pub fn count(&self, flag: usize) -> u32 {
        self.counts.get(flag).copied().unwrap_or_default()
    }
}

/// A test the runner lists: its name, as declared, and its kind (`std.testing.Kind`).
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Listed {
    pub name: String,
    pub kind: String,
}

/// An argument of a property's smallest failing case (`std.testing.Argument`): its parameter's
/// name, and its value as its type writes it as JSON.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Argument {
    pub name: String,
    pub value: vun_data::Value<'static>,
}

/// A test run: its name, as declared, what became of it, and how long it took in nanoseconds
/// (`None` where the system gave the program no clock).
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Ran {
    pub name: String,
    pub outcome: Outcome,
    pub nanos: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub enum Outcome {
    Passed,
    Ignored(Ignored),
    Trapped(Trapped),
    Falsified(Falsified),
    Measured(Measured),
    Fuzzed(Fuzzed),
}

impl Outcome {
    /// The trap that ended the test, or the smallest failing case of a property.
    #[must_use]
    pub fn trap(&self) -> Option<&Trapped> {
        match self {
            Self::Trapped(trapped) | Self::Falsified(Falsified { trapped, .. }) => Some(trapped),
            Self::Passed | Self::Ignored(_) | Self::Measured(_) | Self::Fuzzed(_) => None,
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

/// What a measured benchmark reports (`std.testing.bench.Measured`): how many times its body ran,
/// and how long that took.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Measured {
    pub iterations: u64,
    pub nanos: u64,
}

impl Measured {
    /// The nanoseconds a run of the body took on average.
    #[must_use]
    pub fn per_iteration(self) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "an average, shown")]
        self.per_iteration_of(self.nanos as f64)
    }

    /// `total`, a quantity measured over all the runs of the body, for one run.
    #[must_use]
    pub fn per_iteration_of(self, total: f64) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "an average, shown")]
        let iterations = self.iterations.max(1) as f64;
        total / iterations
    }
}

/// What fuzzing a property did when no case trapped (`std.testing.fuzz.Fuzzed`): how many cases
/// ran, how many streams of choices it kept, and how many coverage flags they set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Fuzzed {
    pub runs: u64,
    pub kept: usize,
    pub flags: usize,
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

/// How many tests passed, trapped, were falsified, were not run, were measured and were fuzzed,
/// and how long the run took in nanoseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ToData, FromData)]
pub struct Totals {
    pub passed: usize,
    pub trapped: usize,
    pub falsified: usize,
    pub ignored: usize,
    pub measured: usize,
    pub fuzzed: usize,
    pub nanos: Option<u64>,
}

/// An argument as a failing case's report shows it: `x = 200`, its value as JSON.
impl std::fmt::Display for Argument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} = {}", self.name, vun_data::json::Json(&self.value))
    }
}

impl Line {
    /// The line the runner wrote as `text`; `None` for what the tests write themselves.
    #[must_use]
    pub fn of(text: &str) -> Option<Self> {
        vun_data::json::parse(text).ok()?.read().ok()
    }
}
