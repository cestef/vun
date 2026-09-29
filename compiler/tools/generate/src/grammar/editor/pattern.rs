//! Regular expressions in the Oniguruma dialect Sublime syntaxes use, composed from pieces rather
//! than written whole, so that each piece comes from the table or lexer constant that owns it.

use std::fmt;

/// A regular expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Pattern(String);

/// Characters that stand for themselves only when escaped.
const META: &str = r"\^$.|?*+()[]{}";

impl Pattern {
    /// A piece of regular expression taken as written.
    pub(super) fn raw(text: &str) -> Self {
        Self(text.to_owned())
    }

    /// `text` matched literally.
    pub(super) fn literal(text: &str) -> Self {
        Self::raw(&text.chars().map(Self::escaped).collect::<String>())
    }

    /// `c` as written inside a pattern, escaped when it means something else.
    fn escaped(c: char) -> String {
        if META.contains(c) { ['\\', c].iter().collect() } else { c.to_string() }
    }

    /// A class of the characters `chars`, each taken literally.
    pub(super) fn class(chars: impl IntoIterator<Item = char>) -> Self {
        let inside: String =
            chars.into_iter().map(|c| if c == '-' { r"\-".to_owned() } else { Self::escaped(c) }).collect();
        Self(["[", &inside, "]"].concat())
    }

    /// Any character but `chars`, each taken literally.
    pub(super) fn except(chars: impl IntoIterator<Item = char>) -> Self {
        let class = Self::class(chars).0;
        Self(["[^", &class[1..]].concat())
    }

    /// Any of `patterns`, as one group; literal ones are best ordered longest first.
    pub(super) fn any(patterns: impl IntoIterator<Item = Self>) -> Self {
        let parts: Vec<String> = patterns.into_iter().map(|p| p.0).collect();
        Self(["(?:", &parts.join("|"), ")"].concat())
    }

    /// Any of the words `words`, each whole: not part of a longer name.
    pub(super) fn words<'w>(words: impl IntoIterator<Item = &'w str>) -> Self {
        let mut words: Vec<&str> = words.into_iter().collect();
        words.sort_by_key(|w| std::cmp::Reverse(w.len()));
        Self::any(words.into_iter().map(Self::literal)).word()
    }

    /// This pattern, then `next`.
    #[must_use]
    pub(super) fn then(self, next: Self) -> Self {
        Self([self.0, next.0].concat())
    }

    /// This pattern or nothing.
    #[must_use]
    pub(super) fn optional(self) -> Self {
        Self(["(?:", &self.0, ")?"].concat())
    }

    /// This pattern any number of times.
    #[must_use]
    pub(super) fn many(self) -> Self {
        Self(["(?:", &self.0, ")*"].concat())
    }

    /// This pattern between `min` and `max` times.
    #[must_use]
    pub(super) fn times(self, min: usize, max: usize) -> Self {
        let count = if min == max { min.to_string() } else { [min.to_string(), max.to_string()].join(",") };
        Self(["(?:", &self.0, "){", &count, "}"].concat())
    }

    /// This pattern where it is not part of a longer word.
    #[must_use]
    pub(super) fn word(self) -> Self {
        Self::raw(r"\b").then(self).then(Self::raw(r"\b"))
    }

    /// This pattern where `after` follows it, which the match leaves out.
    #[must_use]
    pub(super) fn before(self, after: &Self) -> Self {
        Self([&self.0, "(?=", &after.0, ")"].concat())
    }

    /// This pattern where `after` does not follow it.
    #[must_use]
    pub(super) fn not_before(self, after: &Self) -> Self {
        Self([&self.0, "(?!", &after.0, ")"].concat())
    }

    /// This pattern where one of `before` precedes it, which the match leaves out. Each of
    /// `before` has one fixed length, as a lookbehind requires.
    #[must_use]
    pub(super) fn after(self, before: &[Self]) -> Self {
        let parts: Vec<&str> = before.iter().map(|p| p.0.as_str()).collect();
        Self(["(?<=", &parts.join("|"), ")", &self.0].concat())
    }

    /// This pattern where none of `before` precedes it.
    #[must_use]
    pub(super) fn not_after(self, before: &Self) -> Self {
        Self(["(?<!", &before.0, ")", &self.0].concat())
    }

    /// This pattern, then the rest of the line.
    #[must_use]
    pub(super) fn through_line_end(self) -> Self {
        self.then(Self::raw(".*$"))
    }

    /// The end of a line.
    pub(super) fn line_end() -> Self {
        Self::raw("$")
    }

    /// Optional white space.
    pub(super) fn space() -> Self {
        Self::raw(r"\s*")
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
