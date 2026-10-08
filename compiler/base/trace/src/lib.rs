//! Logging, spans and timings for every compiler crate.
//!
//! Crates log through the re-exported [`tracing`], so the dependency is declared once. What is
//! shown is chosen at run time:
//!
//! - `VUN_LOG` takes directives, such as `vun_sema=debug,vun_mir_passes=trace` or `info`;
//!   without it nothing is logged;
//! - `VUN_LOG_FORMAT` is `compact` (default) or `tree`;
//! - `VUN_TIMINGS=1` times every stage and query (computed, or its saved values read), reported at exit ([`Timings`]);
//! - `VUN_TIMELINE=<file>` also records every run of each, written to the file at exit for
//!   Perfetto ([`Timeline`]);
//! - `VUN_COLOR` (`auto`, `always`, `never`) decides whether log lines are styled.
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
use vun_term::{Color, ColorChoice, Stream};

pub use filter::Filter;
pub use timings::{Activity, Kind, Live, Relay, Relaying, Report, Row, Timeline, Timer, Timings};

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
    /// Where to write the timeline of every activity run, if anywhere.
    pub timeline: Option<std::path::PathBuf>,
    /// Whether log lines are styled: as standard error decides under `VUN_COLOR`, which the
    /// logs read since they start before the command line is.
    pub color: Color,
}

impl Config {
    pub const FILTER_VAR: &'static str = "VUN_LOG";
    pub const FORMAT_VAR: &'static str = "VUN_LOG_FORMAT";
    pub const TIMINGS_VAR: &'static str = "VUN_TIMINGS";
    pub const TIMELINE_VAR: &'static str = "VUN_TIMELINE";
    pub const COLOR_VAR: &'static str = "VUN_COLOR";

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
            timeline: var(Self::TIMELINE_VAR).filter(|v| !v.is_empty()).map(Into::into),
            color: Stream::Stderr.color(ColorChoice::named(var(Self::COLOR_VAR).as_deref())),
        }
    }

    /// The logs `-v` flags ask for, when `VUN_LOG` does not decide: none for one (a flag shows
    /// steps, not logs), then `info`, `debug` and `trace`.
    #[must_use]
    pub fn verbose(flags: usize) -> Option<Self> {
        const LEVELS: [&str; 3] = ["info", "debug", "trace"];
        let level = LEVELS[flags.checked_sub(2)?.min(LEVELS.len() - 1)];
        let config = Self::from_env();
        config.filter.is_none().then(|| Self { filter: Some(level.to_owned()), ..config })
    }

    /// Starts timing and recording the timeline if asked, and installs the global subscriber if a filter asks for logs:
    /// logging is opt-in, and without a subscriber a log costs one check.
    pub fn install(&self) {
        if self.timings {
            Timings::enable();
        }
        if let Some(path) = &self.timeline {
            Timeline::record_to(path);
        }
        if self.filter.is_none() {
            return;
        }
        // A subscriber may already be installed by an embedding tool; keep that one.
        let _ = tracing::subscriber::set_global_default(subscriber::Writer::new(self));
    }
}
