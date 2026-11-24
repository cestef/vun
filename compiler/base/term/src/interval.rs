//! A length of time as a person writes it, with its unit: `15m`, `900 s`, `250 ms`.

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use vun_variants::Variants;

/// A unit an [`Interval`] is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub enum TimeUnit {
    Millis,
    Seconds,
    Minutes,
    Hours,
}

impl TimeUnit {
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Millis => "ms",
            Self::Seconds => "s",
            Self::Minutes => "m",
            Self::Hours => "h",
        }
    }

    #[must_use]
    pub const fn millis(self) -> u64 {
        match self {
            Self::Millis => 1,
            Self::Seconds => 1000,
            Self::Minutes => 60 * 1000,
            Self::Hours => 60 * 60 * 1000,
        }
    }

    fn named(symbol: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|unit| unit.symbol() == symbol)
    }
}

/// A whole number of one [`TimeUnit`], written `15m`, or with a space, `900 s`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    pub count: u64,
    pub unit: TimeUnit,
}

impl Interval {
    #[must_use]
    pub const fn new(count: u64, unit: TimeUnit) -> Self {
        Self { count, unit }
    }

    fn millis(self) -> Option<u64> {
        self.count.checked_mul(self.unit.millis())
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.count, self.unit.symbol())
    }
}

impl From<Interval> for Duration {
    fn from(interval: Interval) -> Self {
        Self::from_millis(interval.millis().unwrap_or(u64::MAX))
    }
}

/// Why a text is no [`Interval`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoInterval;

impl fmt::Display for NoInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a duration is a whole number and a unit, such as ")?;
        for (i, unit) in TimeUnit::ALL.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", Interval::new(15, *unit))?;
        }
        Ok(())
    }
}

impl Error for NoInterval {}

impl FromStr for Interval {
    type Err = NoInterval;

    fn from_str(text: &str) -> Result<Self, NoInterval> {
        let text = text.trim();
        let (number, symbol) = text.split_at(text.find(|c: char| !c.is_ascii_digit()).ok_or(NoInterval)?);
        let count = number.parse().map_err(|_| NoInterval)?;
        let unit = TimeUnit::named(symbol.trim_start()).ok_or(NoInterval)?;
        let interval = Self { count, unit };
        interval.millis().map(|_| interval).ok_or(NoInterval)
    }
}
