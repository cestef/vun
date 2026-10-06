//! Quantities as cargo shows them: `0.42s`, `1m 03s`, `3.4 MiB`, `2 errors`, and a time measured
//! in nanoseconds, `12.3 ns`, a count of instructions, `4.56 M instr`, and a percentage, `3.5%`.

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

/// A time taken: seconds with two decimals, minutes past a minute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Elapsed(pub Duration);

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const MINUTE: u64 = 60;
        let secs = self.0.as_secs();
        if secs < MINUTE {
            write!(f, "{:.2}s", self.0.as_secs_f64())
        } else {
            write!(f, "{}m {:02}s", secs / MINUTE, secs % MINUTE)
        }
    }
}

/// A time measured in nanoseconds, such as a benchmark's per iteration, in the largest unit it
/// reaches: `12.3 ns`, `4.56 µs`, `1.20 ms`, `2.00 s`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nanos(pub f64);

impl fmt::Display for Nanos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNITS: [(&str, f64); 3] = [("s", 1e9), ("ms", 1e6), ("µs", 1e3)];
        match UNITS.into_iter().find(|&(_, size)| self.0 >= size) {
            Some((unit, size)) => write!(f, "{:.2} {unit}", self.0 / size),
            None => write!(f, "{:.1} ns", self.0),
        }
    }
}

impl From<u64> for Nanos {
    fn from(nanos: u64) -> Self {
        #[expect(clippy::cast_precision_loss, reason = "a time, shown with three digits")]
        Self(nanos as f64)
    }
}

/// A count of instructions, such as a benchmark's per iteration, in the largest unit it reaches:
/// `812 instr`, `4.56 k instr`, `1.20 M instr`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instructions(pub f64);

impl fmt::Display for Instructions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNITS: [(&str, f64); 3] = [("G", 1e9), ("M", 1e6), ("k", 1e3)];
        match UNITS.into_iter().find(|&(_, size)| self.0 >= size) {
            Some((unit, size)) => write!(f, "{:.2} {unit} instr", self.0 / size),
            None => write!(f, "{:.1} instr", self.0),
        }
    }
}

/// A ratio in percent, written `3`, `2.5` or `3%`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Percent(pub f64);

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.0)
    }
}

/// Why a text is no [`Percent`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoPercent;

impl fmt::Display for NoPercent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a percentage is a number of at least 0, such as 3 or 2.5%")
    }
}

impl Error for NoPercent {}

impl FromStr for Percent {
    type Err = NoPercent;

    fn from_str(text: &str) -> Result<Self, NoPercent> {
        let number = text.strip_suffix('%').unwrap_or(text);
        number.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0).map(Self).ok_or(NoPercent)
    }
}

/// A size in bytes, in the largest binary unit it reaches, with one decimal past bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bytes(pub u64);

impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
        const STEP: f64 = 1024.0;
        #[expect(clippy::cast_precision_loss, reason = "shown with one decimal")]
        let mut size = self.0 as f64;
        let mut unit = 0;
        while size >= STEP && unit < UNITS.len() - 1 {
            size /= STEP;
            unit += 1;
        }
        if unit == 0 { write!(f, "{} {}", self.0, UNITS[0]) } else { write!(f, "{size:.1} {}", UNITS[unit]) }
    }
}

/// A noun by its singular and plural.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Noun {
    pub one: &'static str,
    pub many: &'static str,
}

impl Noun {
    /// `n` of these: `1 error`, `2 errors`.
    #[must_use]
    pub fn count(self, n: usize) -> Counted {
        Counted(n, self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counted(usize, Noun);

impl fmt::Display for Counted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(n, noun) = *self;
        write!(f, "{n} {}", if n == 1 { noun.one } else { noun.many })
    }
}
