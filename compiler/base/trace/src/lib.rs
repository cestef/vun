//! Logging, spans and timings for every compiler crate.
//!
//! Crates log through the re-exported [`tracing`], so the dependency is declared once. What is
//! shown is chosen at run time:
//!
//! - `OVERT_LOG` takes a `tracing` filter, such as `overt_sema=debug,overt_mir_passes=trace`;
//! - `OVERT_LOG_FORMAT` is `compact` (default) or `tree`;
//! - `OVERT_TIMINGS=1` reports the duration of every closed span, which gives per-stage timings.

pub use tracing;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

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
    pub const FILTER_VAR: &'static str = "OVERT_LOG";
    pub const FORMAT_VAR: &'static str = "OVERT_LOG_FORMAT";
    pub const TIMINGS_VAR: &'static str = "OVERT_TIMINGS";

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
}

/// Installs the global subscriber. Without a filter, only warnings and errors are shown.
pub fn init(config: &Config) {
    let filter = config.filter.as_deref().map_or_else(|| EnvFilter::new("warn"), EnvFilter::new);
    let spans = if config.timings { FmtSpan::CLOSE } else { FmtSpan::NONE };
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(spans)
        .with_writer(std::io::stderr)
        .with_target(true);
    // A subscriber may already be installed by an embedding tool; keep that one.
    let _ = match config.format {
        Format::Compact => builder.compact().try_init(),
        Format::Tree => builder.pretty().try_init(),
    };
}
