//! What a benchmark is measured in (spec 11, section 5).

use vun_data::{FromData, ToData};
use vun_term::{Instructions, Nanos};
use vun_variants::Variants;

/// What `--measure` chooses between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Variants, ToData, FromData)]
pub enum Measure {
    /// Wall-clock time per iteration.
    #[default]
    Time,
    /// Instructions the process retired per iteration, where the platform counts them.
    Instructions,
}

impl Measure {
    /// How `--measure` writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Instructions => "instructions",
        }
    }

    /// The names `--measure` takes.
    #[must_use]
    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|measure| measure.name()).collect()
    }

    /// The measure `name` writes.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|measure| measure.name() == name)
    }

    /// `value`, a quantity of this measure, as people read it.
    #[must_use]
    pub fn show(self, value: f64) -> String {
        match self {
            Self::Time => Nanos(value).to_string(),
            Self::Instructions => Instructions(value).to_string(),
        }
    }
}
