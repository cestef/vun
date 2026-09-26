//! `VUN_LOG` filters: comma-separated directives, each a level (`debug`) or a target and a
//! level (`vun_sema=trace`). The directive with the longest target that prefixes an event's
//! target decides; a directive without a target applies everywhere, and without any, nothing is
//! shown.

use tracing::Level;

/// Separates directives, and a target from its level.
const DIRECTIVES: char = ',';
const TARGET: char = '=';

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

impl Filter {
    /// Reads directives, skipping those whose level does not read.
    #[must_use]
    pub fn parse(text: &str) -> Self {
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

    /// Whether an event or span of `level` from `target` is shown.
    #[must_use]
    pub fn enabled(&self, target: &str, level: Level) -> bool {
        let applies = |d: &&Directive| d.target.as_deref().is_none_or(|t| target.starts_with(t));
        // `max_by_key` keeps the last maximum, so later directives win among equally specific ones.
        let decisive =
            self.directives.iter().filter(applies).max_by_key(|d| d.target.as_ref().map_or(0, String::len));
        decisive.is_some_and(|d| level <= d.level)
    }
}
