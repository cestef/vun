//! Quantities as cargo shows them: `0.42s`, `1m 03s`, `3.4 MiB`, `2 errors`.

use std::fmt;
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
