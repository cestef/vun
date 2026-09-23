//! "Did you mean" suggestions for misspelled names: commands, options, bindings, lints.

/// A name counts as a misspelling of a candidate when at most this fraction of its characters,
/// rounded up, must change.
const TYPO_DIVISOR: usize = 3;

/// The candidate closest to `name` by edit distance, if it is close enough to be a typo.
pub fn closest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let limit = name.chars().count().div_ceil(TYPO_DIVISOR);
    candidates
        .into_iter()
        .map(|c| (distance(name, c), c))
        .filter(|&(d, _)| d <= limit)
        .min_by_key(|&(d, _)| d)
        .map(|(_, c)| c)
}

/// Finding an item of a table by name, or else the name closest to the one asked for.
pub trait Lookup<T> {
    /// The item whose `key` is `name`.
    ///
    /// # Errors
    /// Returns the closest name, if one is close enough, when no item is named `name`.
    fn lookup<'t>(&'t self, name: &str, key: impl Fn(&'t T) -> &'t str) -> Result<&'t T, Option<&'t str>>;
}

impl<T> Lookup<T> for [T] {
    fn lookup<'t>(&'t self, name: &str, key: impl Fn(&'t T) -> &'t str) -> Result<&'t T, Option<&'t str>> {
        self.iter().find(|t| key(t) == name).ok_or_else(|| closest(name, self.iter().map(&key)))
    }
}

/// Levenshtein distance over characters.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let substitute = diagonal + usize::from(ca != cb);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(diagonal + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests;
