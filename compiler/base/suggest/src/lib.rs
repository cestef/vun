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
