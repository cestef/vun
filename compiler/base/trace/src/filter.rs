//! `VUN_LOG` filters: comma-separated directives, each a level (`debug`) or a target and a
//! level (`vun_sema=trace`). The directive with the longest target that prefixes an event's
//! target decides; a directive without a target applies everywhere, and without any, nothing is
//! shown.

use tracing::Level;

/// Separates directives, and a target from its level.
const DIRECTIVES: char = ',';
const TARGET: char = '=';
/// Separates the modules of a target, as Rust paths do.
const PATH: &str = "::";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Directive {
    /// `None` applies to every target.
    target: Option<String>,
    level: Level,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    directives: Vec<Directive>,
}

/// Shows nothing.
impl Default for Filter {
    fn default() -> Self {
        Self { directives: Vec::new() }
    }
}

/// Reads directives, skipping those whose level does not read.
impl From<&str> for Filter {
    fn from(text: &str) -> Self {
        let directives = text.split(DIRECTIVES).map(str::trim).filter(|d| !d.is_empty()).filter_map(|d| {
            match d.split_once(TARGET) {
                Some((target, level)) => {
                    Some(Directive { target: Some(target.to_owned()), level: level.parse().ok()? })
                }
                None => Some(Directive { target: None, level: d.parse().ok()? }),
            }
        });
        let mut filter = Self::default();
        filter.directives.extend(directives);
        filter
    }
}

impl Filter {
    /// Whether an event or span of `level` from `target` is shown.
    #[must_use]
    pub fn enabled(&self, target: &str, level: Level) -> bool {
        // A directive names a module and the modules within it, not every name it begins.
        let within =
            |t: &str| target.strip_prefix(t).is_some_and(|rest| rest.is_empty() || rest.starts_with(PATH));
        let applies = |d: &&Directive| d.target.as_deref().is_none_or(within);
        // `max_by_key` keeps the last maximum, so later directives win among equally specific ones.
        let decisive =
            self.directives.iter().filter(applies).max_by_key(|d| d.target.as_ref().map_or(0, String::len));
        decisive.is_some_and(|d| level <= d.level)
    }
}
