//! Logging, spans and timings for every compiler crate.
//!
//! Crates log through the re-exported [`tracing`], so the dependency is declared once. What is
//! shown is chosen at run time:
//!
//! - `VUN_LOG` takes directives, such as `vun_sema=debug,vun_mir_passes=trace` or `info`;
//!   without it nothing is logged;
//! - `VUN_LOG_FORMAT` is `compact` (default) or `tree`;
//! - `VUN_TIMINGS=1` times every stage and query, reported at exit ([`Timings`]).
//!
//! Levels say how much a log line stands for, the same in every crate:
//!
//! - `error` and `warn`: something went wrong that the compiler went on past, not a diagnostic
//!   (a cache that cannot be written);
//! - `info`: one line per step of a run: a file loaded, a stage done, queries loaded or saved,
//!   a program built or linked;
//! - `debug`: one line per unit of work: a query computed and why, a package read, a function
//!   built, lowered or generated, a tool run;
//! - `trace`: one line per element: a statement, an instruction, an origin.

mod filter;
mod subscriber;
#[cfg(test)]
mod tests;
mod timings;

pub use tracing;

pub use filter::Filter;
pub use timings::{Activity, Kind, Report, Row, Timer, Timings};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// One line per event.
    #[default]
    Compact,
    /// Events indented under the spans they occur in.
    Tree,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub filter: Option<String>,
    pub format: Format,
    pub timings: bool,
}

impl Config {
    pub const FILTER_VAR: &'static str = "VUN_LOG";
    pub const FORMAT_VAR: &'static str = "VUN_LOG_FORMAT";
    pub const TIMINGS_VAR: &'static str = "VUN_TIMINGS";

    /// Reads the configuration from the environment.
    #[must_use]
    pub fn from_env() -> Self {
        let var = |name| std::env::var(name).ok();
        Self {
            filter: var(Self::FILTER_VAR),
            format: match var(Self::FORMAT_VAR).as_deref() {
                Some("tree") => Format::Tree,
                _ => Format::Compact,
            },
            timings: var(Self::TIMINGS_VAR).is_some_and(|v| v == "1"),
        }
    }

    /// Starts timing if asked, and installs the global subscriber if a filter asks for logs:
    /// logging is opt-in, and without a subscriber a log costs one check.
    pub fn install(&self) {
        if self.timings {
            Timings::enable();
        }
        if self.filter.is_none() {
            return;
        }
        // A subscriber may already be installed by an embedding tool; keep that one.
        let _ = tracing::subscriber::set_global_default(subscriber::Writer::new(self));
    }
}
