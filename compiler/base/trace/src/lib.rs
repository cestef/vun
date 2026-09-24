//! Logging, spans and timings for every compiler crate.
//!
//! Crates log through the re-exported [`tracing`], so the dependency is declared once. What is
//! shown is chosen at run time:
//!
//! - `VUN_LOG` takes directives, such as `vun_sema=debug,vun_mir_passes=trace` or `info`;
//! - `VUN_LOG_FORMAT` is `compact` (default) or `tree`;
//! - `VUN_TIMINGS=1` reports the duration of every closed span, which gives per-stage timings.

mod filter;
mod subscriber;
#[cfg(test)]
mod tests;

pub use tracing;

pub use filter::Filter;

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

    /// Installs the global subscriber. Without a filter, only warnings and errors are shown.
    pub fn install(&self) {
        // A subscriber may already be installed by an embedding tool; keep that one.
        let _ = tracing::subscriber::set_global_default(subscriber::Writer::new(self));
    }
}
